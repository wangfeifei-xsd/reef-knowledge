use serde::{Deserialize, Serialize};

use super::TaskUsage;

/// wiki 双路召回共用参数（与是否调用 LLM 无关）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueRecallBaseParams {
    pub query: String,
    #[serde(default)]
    pub wiki_prefix: String,
    #[serde(default = "default_max_files")]
    pub max_files: u32,
    #[serde(default = "default_bm25_top_n")]
    pub bm25_top_n: u32,
    #[serde(default = "default_vector_top_n")]
    pub vector_top_n: u32,
    #[serde(default = "default_top_k")]
    pub top_k_chunks: u32,
    #[serde(default = "default_chunk_max_chars")]
    pub chunk_max_chars: u32,
    #[serde(default = "default_context_budget")]
    pub context_budget_chars: u32,
}

fn default_max_files() -> u32 {
    80
}
fn default_bm25_top_n() -> u32 {
    10
}
fn default_vector_top_n() -> u32 {
    10
}
fn default_top_k() -> u32 {
    6
}
fn default_chunk_max_chars() -> u32 {
    1200
}
fn default_context_budget() -> u32 {
    12000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueRecallRequest {
    #[serde(flatten)]
    pub base: DialogueRecallBaseParams,
}

/// 多轮会话中已发生的 user / assistant 消息（不含当前轮用户问题；不持久化，仅本次请求带给 LLM）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueChatTurn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueRecallTestRequest {
    #[serde(flatten)]
    pub base: DialogueRecallBaseParams,
    /// 覆盖默认 system 提示；为空则用内置问答约束
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// `role` 仅接受 `user` 或 `assistant`（大小写不敏感），其余条目不注入。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conversation_history: Vec<DialogueChatTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueRecallHit {
    pub path: String,
    pub score: f64,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueRecallResponse {
    pub user_query: String,
    pub recall_method: String,
    pub query_terms: Vec<String>,
    pub files_scanned: u32,
    pub recall_hits: Vec<DialogueRecallHit>,
    pub injected_context: String,
    pub context_truncated: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueRecallTestResponse {
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TaskUsage>,
    pub user_query: String,
    pub recall_method: String,
    pub query_terms: Vec<String>,
    pub files_scanned: u32,
    pub recall_hits: Vec<DialogueRecallHit>,
    pub injected_context: String,
    pub context_truncated: bool,
    pub assistant_reply: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecallStopwordsUpdateRequest {
    #[serde(default)]
    pub words: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallStopwordsResponse {
    pub words: Vec<String>,
    pub source: String,
    pub runtime_path: String,
    pub count: u32,
    pub message: String,
}
