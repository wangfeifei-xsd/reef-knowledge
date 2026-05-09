//! 复刻 Python `app/services/llm_tasks.py`：compile / lint / polish 三个任务。

use async_openai::types::{
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};

use crate::config::Settings;
use crate::error::{AppError, AppResult};
use crate::llm::client::build_openai_client;
use crate::llm::strip::strip_think_blocks;
use crate::models::{
    CompileTaskRequest, CompileTaskResponse, LayerName, LintTaskRequest, LintTaskResponse,
    PolishTextRequest, PolishTextResponse, TaskUsage,
};
use crate::storage;
use crate::vector_index;

fn usage_from(usage: Option<async_openai::types::CompletionUsage>) -> Option<TaskUsage> {
    usage.map(|u| TaskUsage {
        prompt_tokens: Some(u.prompt_tokens),
        completion_tokens: Some(u.completion_tokens),
        total_tokens: Some(u.total_tokens),
    })
}

pub async fn run_compile(
    settings: &Settings,
    body: CompileTaskRequest,
) -> AppResult<CompileTaskResponse> {
    let built = build_openai_client(settings)?;
    let data_root = settings.data_root.clone();
    storage::ensure_layer_tree(&data_root)?;

    // 规范层
    let mut schema_chunks: Vec<String> = Vec::new();
    let mut schema_paths = body.schema_paths.clone().unwrap_or_default();
    let agents_path = storage::layer_root(&data_root, LayerName::Schema).join("AGENTS.md");
    if agents_path.is_file() && !schema_paths.iter().any(|p| p == "AGENTS.md") {
        schema_paths.insert(0, "AGENTS.md".to_string());
    }
    for sp in &schema_paths {
        let (text, _) = storage::read_file(&data_root, LayerName::Schema, sp, settings.max_file_bytes)?;
        schema_chunks.push(format!("### 规范文件: {sp}\n\n{text}"));
    }

    // 原始层
    let mut raw_chunks: Vec<String> = Vec::new();
    for rp in &body.input_paths {
        let (text, _) = storage::read_file(&data_root, LayerName::Raw, rp, settings.max_file_bytes)?;
        raw_chunks.push(format!("### 原始素材: {rp}\n\n{text}"));
    }

    let system = "你是知识库编译助手。根据「规范层」约束，将「原始层」材料整理为结构化 Markdown wiki 条目，\
保持标题层级、内部链接与交叉引用清晰。只输出最终 wiki 正文（Markdown），不要代码围栏包裹全文。\
不要在正文中输出推理思考标签（如 think/reasoning）或「思考」包裹块。";

    let mut user_parts: Vec<String> = Vec::new();
    user_parts.push(if schema_chunks.is_empty() {
        "(未提供规范文件)".to_string()
    } else {
        schema_chunks.join("\n\n")
    });
    user_parts.push(raw_chunks.join("\n\n"));
    // 与 Python `if body.extra_instructions:` 严格一致：仅排除 None 和空字符串。
    if let Some(extra) = body.extra_instructions.as_deref() {
        if !extra.is_empty() {
            user_parts.push(format!("附加说明:\n{extra}"));
        }
    }
    let user = format!(
        "请将以上内容编译为单篇 wiki 文档，输出路径目标为: {}\n\n---\n\n{}",
        body.output_path,
        user_parts.join("\n\n---\n\n")
    );

    let req = CreateChatCompletionRequestArgs::default()
        .model(&built.effective.model)
        .max_tokens(built.effective.max_tokens)
        .messages([
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system)
                .build()
                .map_err(|e| AppError::Internal(format!("system msg 构建失败：{e}")))?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(user)
                .build()
                .map_err(|e| AppError::Internal(format!("user msg 构建失败：{e}")))?
                .into(),
        ])
        .build()
        .map_err(|e| AppError::Internal(format!("chat req 构建失败：{e}")))?;

    let completion = built.client.chat().create(req).await?;
    let content_raw = completion
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default()
        .trim()
        .to_string();
    let content = strip_think_blocks(&content_raw);
    if content.is_empty() {
        return Err(AppError::BadGateway(
            "模型返回空内容，或去除 think / reasoning 等思考块后无可用正文".to_string(),
        ));
    }

    storage::write_file(
        &data_root,
        LayerName::Wiki,
        &body.output_path,
        &content,
        settings.max_file_bytes,
    )?;
    vector_index::delete_wiki_vectors(&data_root, &body.output_path)?;
    Ok(CompileTaskResponse {
        model: built.effective.model.clone(),
        usage: usage_from(completion.usage),
        output_path: body.output_path.clone(),
        written_files: vec![body.output_path],
        message: "编译完成并已写入编译层".to_string(),
    })
}

fn collect_wiki_markdown(
    data_root: &std::path::Path,
    rel_prefix: &str,
    max_files: u32,
    max_bytes: u64,
) -> AppResult<Vec<(String, String)>> {
    let base = storage::layer_root(data_root, LayerName::Wiki);
    let root = if rel_prefix.is_empty() {
        base.clone()
    } else {
        storage::safe_resolve_under(&base, rel_prefix)?
    };
    let mut out = Vec::<(String, String)>::new();
    if root.is_file() {
        let (text, _) = storage::read_file(data_root, LayerName::Wiki, rel_prefix, max_bytes)?;
        out.push((rel_prefix.to_string(), text));
        return Ok(out);
    }
    let mut paths: Vec<std::path::PathBuf> = walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("md"))
        .collect();
    paths.sort();
    for path in paths {
        if out.len() >= max_files as usize {
            break;
        }
        let rel = path.strip_prefix(&base).unwrap_or(&path);
        let rel_posix = rel
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => Some(s.to_string_lossy().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/");
        let (text, _) = storage::read_file(data_root, LayerName::Wiki, &rel_posix, max_bytes)?;
        out.push((rel_posix, text));
    }
    Ok(out)
}

pub async fn run_lint(settings: &Settings, body: LintTaskRequest) -> AppResult<LintTaskResponse> {
    let built = build_openai_client(settings)?;
    let data_root = settings.data_root.clone();
    storage::ensure_layer_tree(&data_root)?;

    let mut files_inspected: Vec<String> = Vec::new();
    let mut bundled: Vec<String> = Vec::new();
    if let Some(paths) = body.wiki_paths.as_ref() {
        for wp in paths {
            let (text, _) =
                storage::read_file(&data_root, LayerName::Wiki, wp, settings.max_file_bytes)?;
            files_inspected.push(wp.clone());
            bundled.push(format!("### {wp}\n\n{text}"));
        }
    } else {
        let pairs = collect_wiki_markdown(&data_root, "", body.max_files, settings.max_file_bytes)?;
        for (rel, text) in pairs {
            files_inspected.push(rel.clone());
            bundled.push(format!("### {rel}\n\n{text}"));
        }
    }

    if bundled.is_empty() {
        return Ok(LintTaskResponse {
            model: built.effective.model.clone(),
            usage: None,
            report: "wiki 层未发现可检查的 Markdown 文件".to_string(),
            files_inspected: Vec::new(),
            auto_fix_applied: false,
        });
    }

    let mut schema_hint = String::new();
    let agents_path = storage::layer_root(&data_root, LayerName::Schema).join("AGENTS.md");
    if agents_path.is_file() {
        let (s, _) = storage::read_file(&data_root, LayerName::Schema, "AGENTS.md", settings.max_file_bytes)?;
        schema_hint = format!("\n\n规范参考 (AGENTS.md):\n{s}");
    }

    let system = "你是知识库一致性检查助手。根据规范，列出编译层（wiki）中链接断裂、标题层级不当、\
术语不一致等问题，输出简洁的中文检查报告（Markdown 列表）。不要编造文件中不存在的内容。";

    let user = format!(
        "待检查文件如下：{}\n\n{}",
        schema_hint,
        bundled.join("\n\n---\n\n")
    );

    let max_tokens: u32 = std::cmp::min(built.effective.max_tokens, 4096);
    let req = CreateChatCompletionRequestArgs::default()
        .model(&built.effective.model)
        .max_tokens(max_tokens)
        .messages([
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system)
                .build()
                .map_err(|e| AppError::Internal(format!("system msg 构建失败：{e}")))?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(user)
                .build()
                .map_err(|e| AppError::Internal(format!("user msg 构建失败：{e}")))?
                .into(),
        ])
        .build()
        .map_err(|e| AppError::Internal(format!("chat req 构建失败：{e}")))?;

    let completion = built.client.chat().create(req).await?;
    let raw = completion
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default()
        .trim()
        .to_string();
    let report = strip_think_blocks(&raw);
    if report.is_empty() {
        return Err(AppError::BadGateway(
            "模型未返回报告，或去除思考块后无内容".to_string(),
        ));
    }

    let mut auto_fix_applied = false;
    if body.auto_fix {
        let report_path = "_lint_report.md";
        let body_text = format!("# Lint 报告\n\n{report}\n");
        storage::write_file(
            &data_root,
            LayerName::Wiki,
            report_path,
            &body_text,
            settings.max_file_bytes,
        )?;
        files_inspected.push(report_path.to_string());
        auto_fix_applied = true;
    }

    Ok(LintTaskResponse {
        model: built.effective.model.clone(),
        usage: usage_from(completion.usage),
        report,
        files_inspected,
        auto_fix_applied,
    })
}

pub async fn run_polish_text(
    settings: &Settings,
    body: PolishTextRequest,
) -> AppResult<PolishTextResponse> {
    let built = build_openai_client(settings)?;
    if body.content.trim().is_empty() {
        return Err(AppError::BadRequest("content 不能为空".to_string()));
    }

    let system = "你是技术文档与知识库规范编辑。请润色用户给出的 Markdown，用于「规范层」文件（如 AGENTS.md）。\n\
要求：层次清晰、语句通顺、列表与标题格式正确；可补充明显的章节引导句，但**不要编造**用户未提供的业务事实或具体数据。\n\
只输出润色后的完整 Markdown 正文，不要用 markdown 代码围栏包裹全文。";

    let mut user = body.content.trim().to_string();
    if let Some(ins) = body.instruction.as_deref() {
        let ins = ins.trim();
        if !ins.is_empty() {
            user = format!("【附加说明】\n{ins}\n\n---\n\n{user}");
        }
    }

    let max_tokens: u32 = std::cmp::min(built.effective.max_tokens, 16000);
    let req = CreateChatCompletionRequestArgs::default()
        .model(&built.effective.model)
        .max_tokens(max_tokens)
        .messages([
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system)
                .build()
                .map_err(|e| AppError::Internal(format!("system msg 构建失败：{e}")))?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(user)
                .build()
                .map_err(|e| AppError::Internal(format!("user msg 构建失败：{e}")))?
                .into(),
        ])
        .build()
        .map_err(|e| AppError::Internal(format!("chat req 构建失败：{e}")))?;

    let completion = built.client.chat().create(req).await?;
    let raw = completion
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default()
        .trim()
        .to_string();
    let out = strip_think_blocks(&raw);
    if out.is_empty() {
        return Err(AppError::BadGateway(
            "模型返回空内容，或去除思考块后无内容".to_string(),
        ));
    }
    Ok(PolishTextResponse {
        content: out,
        model: built.effective.model.clone(),
        usage: usage_from(completion.usage),
    })
}
