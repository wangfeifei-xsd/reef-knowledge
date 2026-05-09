//! 应用全局状态：持有有效配置 `Settings`，由 Tauri `manage()` 托管。

use parking_lot::RwLock;
use std::sync::Arc;

use crate::config::Settings;

/// 应用全局状态。Settings 在运行期通过环境变量 / `.pathy/llm.json` / 密钥文件等
/// 多源合并；为支持运行时刷新，这里用 `RwLock` 包裹。
pub struct AppState {
    inner: RwLock<Arc<Settings>>,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        Self {
            inner: RwLock::new(Arc::new(settings)),
        }
    }

    /// 读取一份不可变快照（克隆 Arc）。
    pub fn settings(&self) -> Arc<Settings> {
        self.inner.read().clone()
    }

    /// 用新的设置覆盖（写设置时使用）。
    pub fn replace(&self, settings: Settings) {
        let mut g = self.inner.write();
        *g = Arc::new(settings);
    }
}
