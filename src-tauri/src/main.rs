// 防止 Windows 编译时弹出额外的控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    reef_knowledge_lib::run();
}
