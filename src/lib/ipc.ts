import { invoke } from "@tauri-apps/api/core";
import type {
  AppErrorPayload,
  BasicModelSettingsResponse,
  BasicModelSettingsUpdateRequest,
  BasicModelSettingsUpdateResult,
  CompileTaskRequest,
  CompileTaskResponse,
  ConfigSummaryResponse,
  DialogueRecallRequest,
  DialogueRecallResponse,
  DialogueRecallTestRequest,
  DialogueRecallTestResponse,
  FileContentResponse,
  HealthResponse,
  LLMConnectionTestRequest,
  LLMSettingsResponse,
  LLMSettingsUpdateRequest,
  LLMSettingsUpdateResult,
  LLMTestResponse,
  LayerFileListResponse,
  LayerName,
  LintTaskRequest,
  LintTaskResponse,
  ListLayerResponse,
  PolishTextRequest,
  PolishTextResponse,
  RecallStopwordsResponse,
  RecallStopwordsUpdateRequest,
  WikiEmbedRequest,
  WikiEmbedResponse,
} from "./types";

/**
 * 统一调用：把 Rust 端 `AppError` 序列化的 `{ status, detail }` 包成 `Error`。
 */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    if (e && typeof e === "object" && "status" in e && "detail" in e) {
      const err = e as AppErrorPayload;
      const wrapper = new Error(`[${err.status}] ${err.detail}`);
      (wrapper as Error & { status?: number }).status = err.status;
      throw wrapper;
    }
    throw e instanceof Error ? e : new Error(String(e));
  }
}

// ---- health & meta ----
export const apiHealth = () => call<HealthResponse>("health");
export const apiGetConfigSummary = () => call<ConfigSummaryResponse>("get_config_summary");

// ---- layers ----
export const apiListEntries = (layer: LayerName, prefix?: string) =>
  call<ListLayerResponse>("list_entries", { layer, prefix });

export const apiListLayerFiles = (
  layer: LayerName,
  suffix?: string,
  maxFiles?: number,
) =>
  call<LayerFileListResponse>("list_layer_files", {
    layer,
    suffix,
    maxFiles,
  });

export const apiReadLayerFile = (layer: LayerName, path: string) =>
  call<FileContentResponse>("read_layer_file", { layer, path });

export const apiWriteLayerFile = (
  layer: LayerName,
  path: string,
  content: string,
) =>
  call<FileContentResponse>("write_layer_file", {
    layer,
    path,
    content,
  });

export const apiUploadLayerFile = (
  layer: LayerName,
  bytes: ArrayBuffer | Uint8Array,
  options?: { path?: string; filename?: string },
) => {
  const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  return call<FileContentResponse>("upload_layer_file", {
    layer,
    path: options?.path,
    filename: options?.filename,
    bytes: Array.from(u8),
  });
};

export const apiDeleteLayerFile = (layer: LayerName, path: string) =>
  call<{ ok: boolean; deleted: string }>("delete_layer_file", {
    layer,
    path,
  });

export const apiArchiveLayer = async (layer: LayerName, prefix?: string) => {
  const bytes = await call<number[]>("archive_layer", { layer, prefix });
  return new Uint8Array(bytes);
};

// ---- llm settings ----
export const apiGetLLMSettings = () => call<LLMSettingsResponse>("get_llm_settings");
export const apiPutLLMSettings = (body: LLMSettingsUpdateRequest) =>
  call<LLMSettingsUpdateResult>("put_llm_settings", { body });
export const apiTestLLMConnection = (body?: LLMConnectionTestRequest) =>
  call<LLMTestResponse>("test_llm_connection", { body });

export const apiGetEmbeddingSettings = () =>
  call<BasicModelSettingsResponse>("get_embedding_settings");
export const apiPutEmbeddingSettings = (body: BasicModelSettingsUpdateRequest) =>
  call<BasicModelSettingsUpdateResult>("put_embedding_settings", { body });
export const apiTestEmbeddingConnection = (body?: LLMConnectionTestRequest) =>
  call<LLMTestResponse>("test_embedding_connection", { body });

export const apiGetRerankSettings = () =>
  call<BasicModelSettingsResponse>("get_rerank_settings");
export const apiPutRerankSettings = (body: BasicModelSettingsUpdateRequest) =>
  call<BasicModelSettingsUpdateResult>("put_rerank_settings", { body });
export const apiTestRerankConnection = (body?: LLMConnectionTestRequest) =>
  call<LLMTestResponse>("test_rerank_connection", { body });

// ---- tasks ----
export const apiTaskCompile = (body: CompileTaskRequest) =>
  call<CompileTaskResponse>("task_compile", { body });
export const apiTaskLint = (body: LintTaskRequest) =>
  call<LintTaskResponse>("task_lint", { body });
export const apiTaskPolishText = (body: PolishTextRequest) =>
  call<PolishTextResponse>("task_polish_text", { body });

// ---- dialogue recall ----
export const apiDialogueRecall = (body: DialogueRecallRequest) =>
  call<DialogueRecallResponse>("dialogue_recall", { body });
export const apiDialogueRecallTest = (body: DialogueRecallTestRequest) =>
  call<DialogueRecallTestResponse>("dialogue_recall_test", { body });
export const apiGetRecallStopwords = () =>
  call<RecallStopwordsResponse>("get_recall_stopwords");
export const apiPutRecallStopwords = (body: RecallStopwordsUpdateRequest) =>
  call<RecallStopwordsResponse>("put_recall_stopwords", { body });

// ---- wiki embedding ----
export const apiEmbedWikiFile = (body: WikiEmbedRequest) =>
  call<WikiEmbedResponse>("embed_wiki_file", { body });
