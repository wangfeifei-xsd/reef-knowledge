use serde::{Deserialize, Serialize};

use super::TaskUsage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileTaskRequest {
    /// 原始层内相对路径列表（如 notes/foo.md）
    pub input_paths: Vec<String>,
    /// 编译层写入路径（相对 wiki）
    pub output_path: String,
    /// 规范层待注入文件相对路径；默认包含 AGENTS.md（若存在）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_paths: Option<Vec<String>>,
    /// 附加编译说明
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileTaskResponse {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TaskUsage>,
    pub output_path: String,
    pub written_files: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintTaskRequest {
    /// 待检查 wiki 相对路径；为空则扫描整个 wiki 层
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wiki_paths: Option<Vec<String>>,
    #[serde(default)]
    pub auto_fix: bool,
    #[serde(default = "default_max_files")]
    pub max_files: u32,
}

fn default_max_files() -> u32 {
    50
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintTaskResponse {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TaskUsage>,
    pub report: String,
    pub files_inspected: Vec<String>,
    pub auto_fix_applied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolishTextRequest {
    /// 待润色的 Markdown 正文
    pub content: String,
    /// 对模型的额外说明（可选）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instruction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolishTextResponse {
    pub content: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TaskUsage>,
}
