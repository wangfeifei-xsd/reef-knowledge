use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiEmbedRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiEmbedResponse {
    pub path: String,
    pub chunk_count: u32,
    pub model: String,
    pub updated_at: String,
    pub message: String,
}
