// 与 Rust 端 models 字段一一对齐。后续可改用 ts-rs 自动生成。

export type LayerName = "raw" | "wiki" | "schema";
export type LLMFieldSource = "env" | "file" | "default";

export interface HealthResponse {
  status: string;
  service: string;
}

export interface ConfigSummaryResponse {
  data_root: string;
  data_root_resolved: string;
  openai_base_url_configured: boolean;
  openai_model: string;
  openai_timeout_seconds: number;
  openai_max_tokens: number;
  openai_api_key_configured: boolean;
  layers: string[];
  auth_enabled: boolean;
}

export interface TaskUsage {
  prompt_tokens?: number;
  completion_tokens?: number;
  total_tokens?: number;
}

export interface DirEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size?: number;
  embedding_status?: string;
}

export interface ListLayerResponse {
  layer: LayerName;
  prefix: string;
  entries: DirEntry[];
}

export interface LayerFileListResponse {
  layer: LayerName;
  paths: string[];
  truncated: boolean;
}

export interface FileContentResponse {
  layer: LayerName;
  path: string;
  content: string;
  size: number;
}

export interface LLMSettingsResponse {
  openai_model: string;
  openai_model_source: LLMFieldSource;
  openai_base_url?: string | null;
  openai_base_url_source: LLMFieldSource;
  openai_timeout_seconds: number;
  openai_timeout_source: LLMFieldSource;
  openai_max_tokens: number;
  openai_max_tokens_source: LLMFieldSource;
  openai_api_key_configured: boolean;
  env_locks: Record<string, boolean>;
  runtime_llm_json: string;
}

export interface LLMSettingsUpdateRequest {
  openai_model?: string;
  openai_base_url?: string;
  openai_timeout_seconds?: number;
  openai_max_tokens?: number;
  openai_api_key?: string;
}

export interface LLMSettingsUpdateResult {
  settings: LLMSettingsResponse;
  warnings: string[];
}

export interface BasicModelSettingsResponse {
  model: string;
  model_source: LLMFieldSource;
  openai_base_url?: string | null;
  openai_base_url_source: LLMFieldSource;
  openai_timeout_seconds: number;
  openai_timeout_source: LLMFieldSource;
  openai_max_tokens: number;
  openai_max_tokens_source: LLMFieldSource;
  openai_api_key_configured: boolean;
  env_locks: Record<string, boolean>;
  runtime_llm_json: string;
}

export interface BasicModelSettingsUpdateRequest {
  model?: string;
  openai_base_url?: string;
  openai_timeout_seconds?: number;
  openai_max_tokens?: number;
  openai_api_key?: string;
}

export interface BasicModelSettingsUpdateResult {
  settings: BasicModelSettingsResponse;
  warnings: string[];
}

export interface LLMConnectionTestRequest {
  openai_model?: string;
  openai_base_url?: string;
}

export interface LLMTestResponse {
  ok: boolean;
  model: string;
  base_url?: string | null;
  elapsed_ms: number;
  message: string;
  usage?: TaskUsage;
  error?: string;
}

export interface CompileTaskRequest {
  input_paths: string[];
  output_path: string;
  schema_paths?: string[];
  extra_instructions?: string;
}

export interface CompileTaskResponse {
  model: string;
  usage?: TaskUsage;
  output_path: string;
  written_files: string[];
  message: string;
}

export interface LintTaskRequest {
  wiki_paths?: string[];
  auto_fix?: boolean;
  max_files?: number;
}

export interface LintTaskResponse {
  model: string;
  usage?: TaskUsage;
  report: string;
  files_inspected: string[];
  auto_fix_applied: boolean;
}

export interface PolishTextRequest {
  content: string;
  instruction?: string;
}

export interface PolishTextResponse {
  content: string;
  model: string;
  usage?: TaskUsage;
}

export interface DialogueRecallBaseParams {
  query: string;
  wiki_prefix?: string;
  max_files?: number;
  bm25_top_n?: number;
  vector_top_n?: number;
  top_k_chunks?: number;
  chunk_max_chars?: number;
  context_budget_chars?: number;
}

export type DialogueRecallRequest = DialogueRecallBaseParams;

/** 与 Rust `DialogueChatTurn` 一致；仅 `user` / `assistant` 会注入 LLM。 */
export interface DialogueChatTurn {
  role: string;
  content: string;
}

export interface DialogueRecallTestRequest extends DialogueRecallBaseParams {
  system_prompt?: string;
  /** 当前轮之前的对话（内存）；不含本轮用户输入。 */
  conversation_history?: DialogueChatTurn[];
}

export interface DialogueRecallHit {
  path: string;
  score: number;
  snippet: string;
}

export interface DialogueRecallResponse {
  user_query: string;
  recall_method: string;
  query_terms: string[];
  files_scanned: number;
  recall_hits: DialogueRecallHit[];
  injected_context: string;
  context_truncated: boolean;
  message: string;
}

export interface DialogueRecallTestResponse {
  model: string;
  usage?: TaskUsage;
  user_query: string;
  recall_method: string;
  query_terms: string[];
  files_scanned: number;
  recall_hits: DialogueRecallHit[];
  injected_context: string;
  context_truncated: boolean;
  assistant_reply: string;
  message: string;
}

export interface RecallStopwordsResponse {
  words: string[];
  source: string;
  runtime_path: string;
  count: number;
  message: string;
}

export interface RecallStopwordsUpdateRequest {
  words: string[];
}

export interface WikiEmbedRequest {
  path: string;
}

export interface WikiEmbedResponse {
  path: string;
  chunk_count: number;
  model: string;
  updated_at: string;
  message: string;
}

export interface AppErrorPayload {
  status: number;
  detail: string;
}
