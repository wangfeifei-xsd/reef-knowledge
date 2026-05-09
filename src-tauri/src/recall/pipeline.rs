//! 召回主流程：BM25 + 向量双路 → 合并 rerank → context 截断。
//! 与 Python `perform_wiki_keyword_recall` / `run_dialogue_recall_only` /
//! `run_dialogue_recall_test` 等价。

use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use async_openai::types::{
    ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestAssistantMessageContent,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestUserMessageArgs, ChatCompletionRequestUserMessageContent,
    CreateChatCompletionRequestArgs,
};

use crate::config::Settings;
use crate::error::{AppError, AppResult};
use crate::llm::client::build_openai_client;
use crate::llm::strip::strip_think_blocks;
use crate::models::{
    DialogueChatTurn, DialogueRecallBaseParams, DialogueRecallHit, DialogueRecallRequest,
    DialogueRecallResponse, DialogueRecallTestRequest, DialogueRecallTestResponse, LayerName,
    TaskUsage,
};
use crate::recall::bm25::{extract_query_terms, filter_terms, score_chunks_bm25};
use crate::recall::chunking::{is_meaningful_body, wiki_indexed_chunks, IndexedChunk};
use crate::recall::merge::{merge_and_rerank, MergedCandidate};
use crate::recall::stopwords::read_effective_stopwords;
use crate::storage;
use crate::vector_index;

const RECALL_METHOD_HYBRID: &str = "hybrid_bm25_vector";

const INJECTED_EMPTY_FOR_LLM: &str =
    "(本次召回未命中 wiki 片段；仍将你的问题发给模型，请其说明依据不足。)";
const INJECTED_EMPTY_RECALL_ONLY: &str = "(本次召回未命中 wiki 片段。)";

#[derive(Debug, Clone)]
pub struct WikiKeywordRecallArtifacts {
    pub user_query: String,
    pub query_terms: Vec<String>,
    pub files_scanned: u32,
    pub recall_hits: Vec<DialogueRecallHit>,
    pub injected_context: String,
    pub context_truncated: bool,
}

fn collect_wiki_pairs(
    data_root: &Path,
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
    let mut out: Vec<(String, String)> = Vec::new();
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

fn format_injected_block(rel: &str, heading_path: &str, body: &str) -> String {
    let b = body.trim();
    if heading_path.is_empty() {
        format!("### {rel}\n\n{b}")
    } else {
        format!("### {rel}\n\n**{heading_path}**\n\n{b}")
    }
}

fn trim_context(blocks: &[String], budget: usize) -> (Vec<String>, bool) {
    if budget == 0 {
        return (Vec::new(), true);
    }
    let mut acc: Vec<String> = Vec::new();
    let mut n = 0usize;
    for b in blocks {
        let blen = b.chars().count();
        if n + blen > budget && !acc.is_empty() {
            return (acc, true);
        }
        acc.push(b.clone());
        n += blen;
        if n >= budget {
            let exceeded = blocks.len() > acc.len();
            return (acc, exceeded);
        }
    }
    (acc, false)
}

/// 与 Python `DialogueRecallBaseParams` 的 `ge`/`le` 校验对齐。
fn validate_params(b: &DialogueRecallBaseParams) -> AppResult<()> {
    fn check_u32(name: &str, v: u32, lo: u32, hi: u32) -> AppResult<()> {
        if v < lo || v > hi {
            return Err(AppError::BadRequest(format!(
                "{name} 需在 [{lo}, {hi}] 范围内（当前 {v}）"
            )));
        }
        Ok(())
    }
    check_u32("max_files", b.max_files, 1, 500)?;
    check_u32("bm25_top_n", b.bm25_top_n, 1, 100)?;
    check_u32("vector_top_n", b.vector_top_n, 1, 100)?;
    check_u32("top_k_chunks", b.top_k_chunks, 1, 32)?;
    check_u32("chunk_max_chars", b.chunk_max_chars, 400, 8000)?;
    check_u32("context_budget_chars", b.context_budget_chars, 2000, 100_000)?;
    Ok(())
}

pub async fn perform_wiki_keyword_recall(
    settings: &Settings,
    body: &DialogueRecallBaseParams,
    empty_injected_text: &str,
) -> AppResult<WikiKeywordRecallArtifacts> {
    let started = Instant::now();
    let q = body.query.trim().to_string();
    if q.is_empty() {
        return Err(AppError::BadRequest("query 不能为空".to_string()));
    }
    validate_params(body)?;
    let data_root = settings.data_root.clone();
    storage::ensure_layer_tree(&data_root)?;

    let raw_terms = extract_query_terms(&q);
    let stopwords: HashSet<String> = read_effective_stopwords(settings).into_iter().collect();
    let terms = filter_terms(&raw_terms, &stopwords);

    tracing::info!(
        query_len = q.chars().count(),
        wiki_prefix = %body.wiki_prefix,
        max_files = body.max_files,
        top_k = body.top_k_chunks,
        bm25_top_n = body.bm25_top_n,
        vector_top_n = body.vector_top_n,
        terms_raw = raw_terms.len(),
        terms_kept = terms.len(),
        "dialogue_recall start"
    );

    let prefix = body.wiki_prefix.trim().to_string();
    let pairs = collect_wiki_pairs(
        &data_root,
        &prefix,
        body.max_files,
        settings.max_file_bytes,
    )?;

    let mut all_chunks: Vec<IndexedChunk> = Vec::new();
    for (rel, full) in &pairs {
        all_chunks.extend(wiki_indexed_chunks(rel, full, body.chunk_max_chars as usize));
    }

    // BM25 路
    let mut bm25_scored = score_chunks_bm25(&all_chunks, &terms);
    bm25_scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.rel.cmp(&b.1.rel))
            .then_with(|| a.1.heading_path.cmp(&b.1.heading_path))
    });
    let bm25_top: Vec<(f64, IndexedChunk)> =
        bm25_scored.into_iter().take(body.bm25_top_n as usize).collect();

    // 向量路（async）
    let vector_top = vector_index::score_chunks_vector(settings, &q, &prefix, body.vector_top_n).await;

    let merged: Vec<MergedCandidate> = merge_and_rerank(&bm25_top, &vector_top, &terms);
    let meaningful: Vec<MergedCandidate> = merged
        .into_iter()
        .filter(|m| is_meaningful_body(&m.chunk.body))
        .collect();
    let top: Vec<MergedCandidate> = meaningful
        .into_iter()
        .take(body.top_k_chunks as usize)
        .collect();

    let mut block_lines: Vec<String> = Vec::new();
    let mut candidates: Vec<(f64, String, String, String)> = Vec::new();
    for it in &top {
        candidates.push((
            it.rerank_score,
            it.chunk.rel.clone(),
            it.chunk.heading_path.clone(),
            it.chunk.body.clone(),
        ));
        block_lines.push(format_injected_block(
            &it.chunk.rel,
            &it.chunk.heading_path,
            &it.chunk.body,
        ));
    }

    let (blocks, ctx_truncated) =
        trim_context(&block_lines, body.context_budget_chars as usize);
    let kept_n = blocks.len();
    let mut hits: Vec<DialogueRecallHit> = Vec::new();
    for (sc, rel, hpath, chunk_body) in candidates.into_iter().take(kept_n) {
        let mut snip = chunk_body.replace("\r\n", "\n").trim().to_string();
        if !hpath.is_empty() {
            snip = format!("{hpath}\n{snip}").trim().to_string();
        }
        let chars: Vec<char> = snip.chars().collect();
        if chars.len() > 320 {
            snip = format!("{}…", chars.iter().take(320).collect::<String>());
        }
        hits.push(DialogueRecallHit {
            path: rel,
            score: round6(sc),
            snippet: snip,
        });
    }

    let injected = if blocks.is_empty() {
        empty_injected_text.to_string()
    } else {
        blocks.join("\n\n---\n\n")
    };

    tracing::info!(
        recall_hits = hits.len(),
        context_truncated = ctx_truncated,
        injected_chars = injected.chars().count(),
        elapsed_ms = started.elapsed().as_secs_f64() * 1000.0,
        "dialogue_recall done"
    );

    Ok(WikiKeywordRecallArtifacts {
        user_query: q,
        query_terms: terms,
        files_scanned: pairs.len() as u32,
        recall_hits: hits,
        injected_context: injected,
        context_truncated: ctx_truncated,
    })
}

fn round6(v: f64) -> f64 {
    (v * 1_000_000.0).round() / 1_000_000.0
}

/// 单条历史消息最大字符数（Unicode 标量个数近似）
const MAX_TURN_CHARS: usize = 16_000;
/// 注入 LLM 的历史总预算（字符近似）
const MAX_HISTORY_CHARS: usize = 32_000;
/// 最多注入的历史条数（user+assistant 各算一条）
const MAX_HISTORY_TURNS: usize = 48;

fn truncate_turn_content(s: &str) -> String {
    let n = s.chars().count();
    if n <= MAX_TURN_CHARS {
        return s.to_string();
    }
    let head: String = s.chars().take(MAX_TURN_CHARS).collect();
    format!("{head}\n…（已截断）")
}

/// 仅保留 user/assistant，去空、截断单条，再按条数与总字数从旧到新裁剪。
fn normalize_conversation_history(raw: Vec<DialogueChatTurn>) -> Vec<DialogueChatTurn> {
    let mut v: Vec<DialogueChatTurn> = Vec::new();
    for t in raw {
        let role = t.role.to_lowercase();
        if role != "user" && role != "assistant" {
            continue;
        }
        let content = t.content.trim();
        if content.is_empty() {
            continue;
        }
        v.push(DialogueChatTurn {
            role,
            content: truncate_turn_content(content),
        });
    }
    if v.len() > MAX_HISTORY_TURNS {
        v = v[v.len() - MAX_HISTORY_TURNS..].to_vec();
    }
    let mut total: usize = v.iter().map(|t| t.content.chars().count()).sum();
    while total > MAX_HISTORY_CHARS && !v.is_empty() {
        let removed = v.remove(0);
        total = total.saturating_sub(removed.content.chars().count());
    }
    v
}

fn history_to_chat_messages(
    history: Vec<DialogueChatTurn>,
) -> AppResult<Vec<ChatCompletionRequestMessage>> {
    let mut out = Vec::with_capacity(history.len());
    for t in history {
        match t.role.as_str() {
            "user" => {
                let m = ChatCompletionRequestUserMessageArgs::default()
                    .content(ChatCompletionRequestUserMessageContent::Text(t.content))
                    .build()
                    .map_err(|e| AppError::Internal(format!("历史 user 消息构建失败：{e}")))?;
                out.push(ChatCompletionRequestMessage::User(m));
            }
            "assistant" => {
                let m = ChatCompletionRequestAssistantMessageArgs::default()
                    .content(ChatCompletionRequestAssistantMessageContent::Text(t.content))
                    .build()
                    .map_err(|e| AppError::Internal(format!("历史 assistant 消息构建失败：{e}")))?;
                out.push(ChatCompletionRequestMessage::Assistant(m));
            }
            _ => {}
        }
    }
    Ok(out)
}

pub async fn run_dialogue_recall_only(
    settings: &Settings,
    body: DialogueRecallRequest,
) -> AppResult<DialogueRecallResponse> {
    let art = perform_wiki_keyword_recall(settings, &body.base, INJECTED_EMPTY_RECALL_ONLY).await?;
    Ok(DialogueRecallResponse {
        user_query: art.user_query,
        recall_method: RECALL_METHOD_HYBRID.to_string(),
        query_terms: art.query_terms,
        files_scanned: art.files_scanned,
        recall_hits: art.recall_hits,
        injected_context: art.injected_context,
        context_truncated: art.context_truncated,
        message: "已完成 wiki 双路召回（BM25 + 向量）并 rerank（未调用 LLM）".to_string(),
    })
}

pub async fn run_dialogue_recall_test(
    settings: &Settings,
    body: DialogueRecallTestRequest,
) -> AppResult<DialogueRecallTestResponse> {
    let art = perform_wiki_keyword_recall(settings, &body.base, INJECTED_EMPTY_FOR_LLM).await?;
    let injected = art.injected_context.clone();

    let system = body
        .system_prompt
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            "你是一位资深海水鱼健康管理与疾病防控顾问，代号「检疫神克隆体」。\n\
用户消息中会附带从本地 wiki 编译层经 BM25 + 向量双路召回并 rerank 得到的「知识库召回片段」；片段可能不完整、顺序打散或含轻微噪声。\n\
\n\
【作答原则】\n\
1. 以召回片段为首要依据：能引用处请把机制讲清楚（生理、药理、水质、病原体与鱼体状态如何勾连），语气专业、笃定而克制；在科学前提下可适当用比喻或现场感描写，让解释好读、好记，避免干巴巴的关键词堆砌。\n\
2. 证据不足或关键参数缺失（如药物浓度、药浴时长、水温、曝气、换水节奏等）时，须明确写出「在现有资料下只能判断到…」「尚需…才能定论」，不得编造数据或虚构文献。\n\
3. 当片段与问题明显无关、或无法从中提炼有效依据时，须明确写出「知识库中未找到依据」；若给出一两句常识性提醒，须标注为常识推断而非库内结论。\n\
4. 若存在多轮对话，请把用户后续补充与先前描述一并纳入，形成一条连贯的分析链，并在末段收束到用户当下最关心的结论或行动建议。\n\
5. 结构建议：可用小标题、短列表或表格组织信息；优先给可操作的检疫/治疗/观察要点，少用空话套话。"
                .to_string()
        });

    let user_msg = format!(
        "【待答问题】\n{}\n\n---\n【知识库召回片段】\n（经检索与合并，可能截断；请严格据此并结合对话上文作答）\n\n{}",
        art.user_query, injected
    );

    let built = build_openai_client(settings)?;
    let max_tokens: u32 = std::cmp::min(built.effective.max_tokens, 4096);

    let history = normalize_conversation_history(body.conversation_history.clone());
    let history_msgs = history_to_chat_messages(history)?;

    let system_msg = ChatCompletionRequestSystemMessageArgs::default()
        .content(system)
        .build()
        .map_err(|e| AppError::Internal(format!("system msg 构建失败：{e}")))?;
    let final_user = ChatCompletionRequestUserMessageArgs::default()
        .content(ChatCompletionRequestUserMessageContent::Text(user_msg))
        .build()
        .map_err(|e| AppError::Internal(format!("user msg 构建失败：{e}")))?;

    let mut messages: Vec<ChatCompletionRequestMessage> =
        Vec::with_capacity(2 + history_msgs.len());
    messages.push(ChatCompletionRequestMessage::System(system_msg));
    messages.extend(history_msgs);
    messages.push(ChatCompletionRequestMessage::User(final_user));

    let req = CreateChatCompletionRequestArgs::default()
        .model(&built.effective.model)
        .max_tokens(max_tokens)
        .messages(messages)
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
    let reply = strip_think_blocks(&raw);
    if reply.is_empty() {
        return Err(AppError::BadGateway(
            "模型返回空内容，或去除 redacted_thinking / 思考块后无可用正文".to_string(),
        ));
    }

    let usage = completion.usage.map(|u| TaskUsage {
        prompt_tokens: Some(u.prompt_tokens),
        completion_tokens: Some(u.completion_tokens),
        total_tokens: Some(u.total_tokens),
    });

    Ok(DialogueRecallTestResponse {
        model: built.effective.model.clone(),
        usage,
        user_query: art.user_query,
        recall_method: RECALL_METHOD_HYBRID.to_string(),
        query_terms: art.query_terms,
        files_scanned: art.files_scanned,
        recall_hits: art.recall_hits,
        injected_context: injected,
        context_truncated: art.context_truncated,
        assistant_reply: reply,
        message: "已完成召回并调用模型（全流程测试）".to_string(),
    })
}
