# 海洋知识库（reef-knowledge）

基于 [Tauri 2](https://tauri.app/) + Rust + React 18 + TypeScript + Tailwind CSS 的跨平台知识库应用。
后端能力 100% 内置，无需任何 Python / HTTP 进程，全部通过 Tauri IPC 直接调用本地 Rust 函数。

## 已实现的服务端能力（与 `pathy-knowledge-server` 等价）

| 模块 | IPC Command |
|------|-------------|
| 健康 | `health` |
| 元数据 | `get_config_summary` |
| 三层存储 | `list_entries`, `list_layer_files`, `read_layer_file`, `write_layer_file`, `upload_layer_file`, `delete_layer_file`, `archive_layer` |
| 模型配置 | `get/put/test_llm_connection`, `get/put/test_embedding_connection`, `get/put/test_rerank_connection` |
| LLM 任务 | `task_compile`, `task_lint`, `task_polish_text` |
| 对话召回 | `dialogue_recall`, `dialogue_recall_test`, `get/put_recall_stopwords` |
| 向量索引 | `embed_wiki_file` |

数据格式（`<DATA_ROOT>/.pathy/wiki_embedding_index.json`、`.pathy/llm.json`、`.pathy/*_api_key`、`raw/`/`wiki/`/`schema/` 三层）与原 Python 服务端 100% 兼容，可直接互相迁移。

## 快速开始

环境要求：

- Node.js 18+（本仓库示例统一用 **`npm`**；`pnpm`/`yarn` 可选，无额外要求）
- Rust 1.77+，并安装 `cargo`、`rustup`
- macOS：Xcode Command Line Tools；Windows：Visual Studio Build Tools；Linux：参考 [Tauri 前置依赖](https://tauri.app/start/prerequisites/)

```bash
cd reef-knowledge

npm install
npm run tauri:dev
```

构建发行包：

```bash
npm run tauri:build
```

## 默认数据目录

- macOS: `~/Library/Application Support/com.reef.knowledge/reef/`
- Windows: `%APPDATA%\com.reef.knowledge\reef\`
- Linux: `~/.local/share/com.reef.knowledge/reef/`

可通过环境变量 `DATA_ROOT` 覆盖。子目录：`raw/` / `wiki/` / `schema/` / `.pathy/`。

## 配置

环境变量与 `pathy-knowledge-server` 完全一致：`OPENAI_API_KEY` / `OPENAI_BASE_URL` / `OPENAI_MODEL` / `EMBEDDING_*` / `RERANK_*` / `OPENAI_TIMEOUT` / `OPENAI_MAX_TOKENS` / `CONFIG_FILE` / `DATA_ROOT` / `API_KEY`。

也支持运行时通过 IPC 写入 `<DATA_ROOT>/.pathy/llm.json` 与 `.pathy/<*>_api_key`。

## 移动端

桌面跑通后，按以下命令初始化移动端：

```bash
npm run tauri -- ios init
npm run tauri -- android init
npm run tauri -- ios dev
npm run tauri -- android dev
```

> 模型推理 100% 走云端 OpenAI 兼容接口，移动端无需本地 GPU/NPU。

## 目录结构

```
reef-knowledge/
├── src/                     # 前端 React 18
│   ├── lib/{ipc,types,utils}.ts
│   ├── styles/index.css
│   ├── App.tsx
│   └── main.tsx
└── src-tauri/
    └── src/
        ├── lib.rs           # Tauri 入口（桌面 + 移动端共享）
        ├── main.rs
        ├── commands/        # 所有 IPC 端点
        ├── config/          # Settings 三级合并
        ├── storage/         # 三层 FS：safe_resolve_under / CRUD / ZIP
        ├── llm/             # OpenAI 兼容客户端 + tasks + 思考块剥离
        ├── recall/          # BM25 / chunking / merge / pipeline
        ├── vector_index/    # 向量索引（与 Python JSON 完全兼容）
        ├── models/          # 序列化结构
        ├── error.rs         # 统一 AppError
        └── state.rs         # 全局 AppState
```
