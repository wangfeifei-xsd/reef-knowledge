use serde::{Deserialize, Serialize};

/// 三层名称，对应 Python `LayerName` 枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerName {
    Raw,
    Wiki,
    Schema,
}

impl LayerName {
    pub fn as_str(&self) -> &'static str {
        match self {
            LayerName::Raw => "raw",
            LayerName::Wiki => "wiki",
            LayerName::Schema => "schema",
        }
    }

    pub fn all() -> [LayerName; 3] {
        [LayerName::Raw, LayerName::Wiki, LayerName::Schema]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
}

impl Default for HealthResponse {
    fn default() -> Self {
        Self {
            status: "ok".to_string(),
            service: "海洋知识库".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// 仅 wiki 文件可用：embedded / stale / not_embedded
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListLayerResponse {
    pub layer: LayerName,
    pub prefix: String,
    pub entries: Vec<DirEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerFileListResponse {
    pub layer: LayerName,
    pub paths: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContentResponse {
    pub layer: LayerName,
    pub path: String,
    pub content: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteRequest {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSummaryResponse {
    pub data_root: String,
    pub data_root_resolved: String,
    pub openai_base_url_configured: bool,
    pub openai_model: String,
    pub openai_timeout_seconds: f64,
    pub openai_max_tokens: u32,
    pub openai_api_key_configured: bool,
    pub layers: Vec<String>,
    /// 与 Python `bool(settings.api_key)` 对齐：仅当用户显式设置 `API_KEY` 时为 true。
    /// 桌面/移动端走 IPC 不暴露 HTTP，鉴权对应用本身无意义，仅作配置可见性回显。
    pub auth_enabled: bool,
}
