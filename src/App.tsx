import { useCallback, useEffect, useMemo, useState } from "react";
import {
  apiDialogueRecallTest,
  apiGetConfigSummary,
  apiGetEmbeddingSettings,
  apiGetLLMSettings,
  apiGetRerankSettings,
  apiHealth,
  apiPutEmbeddingSettings,
  apiPutLLMSettings,
  apiPutRerankSettings,
  apiTestEmbeddingConnection,
  apiTestLLMConnection,
  apiTestRerankConnection,
} from "./lib/ipc";
import type {
  BasicModelSettingsResponse,
  ConfigSummaryResponse,
  DialogueChatTurn,
  HealthResponse,
  LLMSettingsResponse,
} from "./lib/types";
import {
  Button,
  Cell,
  CellGroup,
  Divider,
  Form,
  Input,
  Navbar,
  TabBar,
  TabBarItem,
  Textarea,
  Toast,
} from "tdesign-mobile-react";
import { AppIcon, ChatBubbleIcon, SettingIcon } from "tdesign-icons-react";

interface Status {
  health?: HealthResponse;
  config?: ConfigSummaryResponse;
  error?: string;
}

type TabKey = "home" | "chat" | "settings";
type SettingsRoute =
  | "root"
  | "model-config-gate"
  | "model-config"
  | "llm"
  | "rerank"
  | "embedding";

const MODEL_CONFIG_GATE_PASSWORD = "332211";

type LlmFormData = {
  openai_model?: string;
  openai_base_url?: string;
  openai_timeout_seconds?: string;
  openai_max_tokens?: string;
  openai_api_key?: string;
};

type BasicFormData = {
  model?: string;
  openai_base_url?: string;
  openai_timeout_seconds?: string;
  openai_max_tokens?: string;
  openai_api_key?: string;
};

export default function App() {
  const [status, setStatus] = useState<Status>({});
  const [tab, setTab] = useState<TabKey>("home");
  const [settingsRoute, setSettingsRoute] = useState<SettingsRoute>("root");
  const [settingsEditing, setSettingsEditing] = useState(false);
  const [opBusy, setOpBusy] = useState(false);

  const [chatMessages, setChatMessages] = useState<DialogueChatTurn[]>([]);
  const [chatInput, setChatInput] = useState("");
  const [chatSending, setChatSending] = useState(false);

  const [modelGatePassword, setModelGatePassword] = useState("");

  const [llmSettings, setLlmSettings] = useState<LLMSettingsResponse | null>(
    null,
  );
  const [embeddingSettings, setEmbeddingSettings] =
    useState<BasicModelSettingsResponse | null>(null);
  const [rerankSettings, setRerankSettings] =
    useState<BasicModelSettingsResponse | null>(null);

  const [llmForm] = Form.useForm();
  const [basicForm] = Form.useForm();

  const refresh = useCallback(async () => {
    try {
      const [health, config] = await Promise.all([
        apiHealth(),
        apiGetConfigSummary(),
      ]);
      setStatus((s) => ({ ...s, health, config, error: undefined }));
    } catch (e) {
      setStatus((s) => ({ ...s, error: (e as Error).message }));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const tabItems = useMemo(
    () => [
      { key: "home" as const, label: "首页", icon: <AppIcon /> },
      { key: "chat" as const, label: "问答", icon: <ChatBubbleIcon /> },
      { key: "settings" as const, label: "设置", icon: <SettingIcon /> },
    ],
    [],
  );

  const navTitle = useMemo(() => {
    if (tab !== "settings") return tab === "home" ? "海洋知识库" : "检疫神克隆体";
    if (settingsRoute === "root") return "设置";
    if (settingsRoute === "model-config-gate") return "模型配置";
    if (settingsRoute === "model-config") return "模型配置";
    if (settingsRoute === "llm") return "LLM 模型配置";
    if (settingsRoute === "rerank") return "Rerank 模型配置";
    return "Embedding 模型配置";
  }, [tab, settingsRoute]);

  const showBack = tab === "settings" && settingsRoute !== "root";
  const onBack = () => {
    if (!showBack) return;
    if (settingsRoute === "llm" || settingsRoute === "embedding" || settingsRoute === "rerank") {
      setSettingsRoute("model-config");
      return;
    }
    if (settingsRoute === "model-config-gate") {
      setModelGatePassword("");
    }
    setSettingsRoute("root");
  };

  useEffect(() => {
    if (tab !== "settings") {
      setSettingsRoute("root");
      setModelGatePassword("");
      setSettingsEditing(false);
      llmForm.reset();
      basicForm.reset();
    }
  }, [tab, llmForm, basicForm]);

  const loadModelSettings = useCallback(
    async (route: SettingsRoute) => {
      try {
        if (route === "llm") {
          const s = await apiGetLLMSettings();
          setLlmSettings(s);
          llmForm.setFieldsValue({
            openai_model: s.openai_model,
            openai_base_url: s.openai_base_url ?? "",
            openai_timeout_seconds: String(s.openai_timeout_seconds),
            openai_max_tokens: String(s.openai_max_tokens),
            openai_api_key: "",
          });
        } else if (route === "embedding") {
          const s = await apiGetEmbeddingSettings();
          setEmbeddingSettings(s);
          basicForm.setFieldsValue({
            model: s.model,
            openai_base_url: s.openai_base_url ?? "",
            openai_timeout_seconds: String(s.openai_timeout_seconds),
            openai_max_tokens: String(s.openai_max_tokens),
            openai_api_key: "",
          });
        } else if (route === "rerank") {
          const s = await apiGetRerankSettings();
          setRerankSettings(s);
          basicForm.setFieldsValue({
            model: s.model,
            openai_base_url: s.openai_base_url ?? "",
            openai_timeout_seconds: String(s.openai_timeout_seconds),
            openai_max_tokens: String(s.openai_max_tokens),
            openai_api_key: "",
          });
        }
      } catch (e) {
        Toast.error({ message: (e as Error).message });
      }
    },
    [llmForm, basicForm],
  );

  useEffect(() => {
    if (tab === "settings" && ["llm", "embedding", "rerank"].includes(settingsRoute)) {
      void loadModelSettings(settingsRoute);
    }
  }, [tab, settingsRoute, loadModelSettings]);

  useEffect(() => {
    setSettingsEditing(false);
    llmForm.reset();
    basicForm.reset();
  }, [settingsRoute, llmForm, basicForm]);

  const renderHome = () => (
    <div className="p-3 md:p-4">
      <CellGroup title="故事里的一页" theme="card">
        <Cell
          title="从备忘到副脑"
          description={
            <span className="text-sm leading-relaxed text-gray-700 dark:text-zinc-300">
              想象你有一排鱼缸：有的刚下检疫、有的在养水、有的已经稳成景。过去这些信息散落在便签、相册和聊天记录里，久了就说不清「那一缸当时到底怎么了」。「海洋知识库」
              想做的，是把这一段水族生活慢慢收进一部可回顾的本地档案。问答里的「检疫神」蒸馏体，只是先探路的那一小步。
            </span>
          }
        />
      </CellGroup>
      <div className="h-3" />
      <CellGroup title="规划中的能力" theme="card">
        <Cell
          title="1. 鱼缸档案"
          description="后续为每一口缸维护独立档案：开缸时间、设备、造景与日常记录串成时间线，让「那一缸」有据可查。"
        />
        <Cell
          title="2. 水质与生物状态"
          description="录入水质参数与缸内生物状态，由模型结合知识库与档案做智能解读：当前风险在哪、优先处理什么、下一步观察或操作给出一组可执行的指导意见。"
        />
        <Cell
          title="3. 鱼只档案与检疫"
          description="为个体或批次维护鱼档案：品种、体长、外观、精神状态、用药与药浴记录；在需要时输出针对性的检疫与用药节奏建议，减少「凭感觉下猛药」的弯路。"
        />
      </CellGroup>
      <div className="h-3" />
      <CellGroup title="当前模型" theme="card">
        <Cell
          title="LLM"
          description={
            status.config ? (
              <span className="font-mono text-xs break-all">
                {status.config.openai_model}
              </span>
            ) : (
              <span className="text-xs text-muted-foreground">加载中…</span>
            )
          }
        />
      </CellGroup>
    </div>
  );

  const sendChat = useCallback(async () => {
    const text = chatInput.trim();
    if (!text || chatSending) return;
    setChatSending(true);
    const prior = chatMessages;
    setChatInput("");
    try {
      const res = await apiDialogueRecallTest({
        query: text,
        wiki_prefix: "",
        conversation_history: prior,
      });
      setChatMessages([
        ...prior,
        { role: "user", content: text },
        { role: "assistant", content: res.assistant_reply },
      ]);
    } catch (e) {
      Toast.error({ message: (e as Error).message });
      setChatInput(text);
    } finally {
      setChatSending(false);
    }
  }, [chatInput, chatMessages, chatSending]);

  const renderChat = () => (
    <div className="flex h-[calc(100dvh-7rem)] flex-col px-3 pt-2 md:h-[calc(100dvh-8rem)]">
      <div className="mb-2 flex shrink-0 justify-end">
        <Button
          size="small"
          variant="outline"
          disabled={chatSending || chatMessages.length === 0}
          onClick={() => setChatMessages([])}
        >
          清空对话
        </Button>
      </div>
      <div className="min-h-0 flex-1 space-y-3 overflow-y-auto pb-2">
        {chatMessages.length === 0 ? (
          <p className="px-1 py-10 text-center text-sm text-gray-500">
            本页为「检疫神 · Chasel」的蒸馏体，请在下方输入问题。
          </p>
        ) : (
          chatMessages.map((m, i) => (
            <div
              key={`${i}-${m.role}`}
              className={m.role === "user" ? "flex justify-end" : "flex justify-start"}
            >
              <div
                className={
                  m.role === "user"
                    ? "max-w-[min(100%,36rem)] rounded-2xl bg-[#0052d9] px-3 py-2 text-left text-sm text-white whitespace-pre-wrap"
                    : "max-w-[min(100%,36rem)] rounded-2xl bg-gray-100 px-3 py-2 text-left text-sm text-gray-900 whitespace-pre-wrap dark:bg-zinc-800 dark:text-zinc-100"
                }
              >
                {m.content}
              </div>
            </div>
          ))
        )}
      </div>
      <div className="shrink-0 border-t border-gray-200 pt-2 pb-1 dark:border-zinc-700">
        <Textarea
          autosize={{ minRows: 1, maxRows: 5 }}
          placeholder="输入问题…"
          value={chatInput}
          disabled={chatSending}
          onChange={(v) => setChatInput(String(v))}
        />
        <div className="mt-2">
          <Button block theme="primary" loading={chatSending} disabled={!chatInput.trim()} onClick={() => void sendChat()}>
            发送
          </Button>
        </div>
      </div>
    </div>
  );

  const renderSettingsRoot = () => (
    <div className="p-3 md:p-4">
      <CellGroup title="功能菜单" theme="card">
        <Cell title="模型配置" arrow onClick={() => setSettingsRoute("model-config-gate")} />
      </CellGroup>
    </div>
  );

  const renderModelConfigGate = () => (
    <div className="p-3 md:p-4">
      <CellGroup title="访问模型配置" theme="card">
        <Cell
          title="请输入密码"
          description={
            <div className="space-y-3 pt-1">
              <Input
                type="password"
                placeholder="密码"
                value={modelGatePassword}
                onChange={(v) => setModelGatePassword(String(v))}
              />
              <Button
                block
                theme="primary"
                onClick={() => {
                  if (modelGatePassword === MODEL_CONFIG_GATE_PASSWORD) {
                    setModelGatePassword("");
                    setSettingsRoute("model-config");
                  } else {
                    Toast.error({ message: "密码错误" });
                  }
                }}
              >
                进入
              </Button>
              <p className="text-center text-xs leading-relaxed text-gray-500 dark:text-zinc-400">
                体验版，免费额度，无需配置
              </p>
            </div>
          }
        />
      </CellGroup>
    </div>
  );

  const renderModelConfigMenu = () => (
    <div className="p-3 md:p-4">
      <p className="mb-3 px-1 text-center text-xs text-gray-500 dark:text-zinc-400">
        体验版，免费额度，无需配置
      </p>
      <CellGroup title="选择配置项" theme="card">
        <Cell title="LLM 模型配置" arrow onClick={() => setSettingsRoute("llm")} />
        <Cell title="Rerank 模型配置" arrow onClick={() => setSettingsRoute("rerank")} />
        <Cell title="Embedding 模型配置" arrow onClick={() => setSettingsRoute("embedding")} />
      </CellGroup>
    </div>
  );

  const renderReadonlyModelCells = (model: string, baseUrl: string, timeout: number, maxTokens: number, keyOk: boolean) => (
    <CellGroup title="当前生效配置" theme="card">
      <Cell title="模型" note={model} />
      <Cell title="Base URL" note={baseUrl} />
      <Cell title="超时（秒）" note={String(timeout)} />
      <Cell title="Max Tokens" note={String(maxTokens)} />
      <Cell title="API Key" note={keyOk ? "已配置" : "未配置"} />
    </CellGroup>
  );

  const renderLlmConfig = () => (
    <div className="p-3 md:p-4">
      {llmSettings
        ? renderReadonlyModelCells(
            llmSettings.openai_model,
            llmSettings.openai_base_url ?? "(默认)",
            llmSettings.openai_timeout_seconds,
            llmSettings.openai_max_tokens,
            llmSettings.openai_api_key_configured,
          )
        : (
          <CellGroup title="当前生效配置" theme="card">
            <Cell title="加载中…" />
          </CellGroup>
        )}

      <div className="h-3 md:h-4" />
      <Button
        block
        variant="outline"
        loading={opBusy}
        onClick={async () => {
          setOpBusy(true);
          try {
            const test = await apiTestLLMConnection({
              openai_model: llmSettings?.openai_model,
              openai_base_url: llmSettings?.openai_base_url ?? undefined,
            });
            if (test.ok) Toast.success({ message: test.message });
            else Toast.error({ message: test.message });
          } catch (e) {
            Toast.error({ message: (e as Error).message });
          } finally {
            setOpBusy(false);
          }
        }}
      >
        测试连通性
      </Button>
      <div className="h-3 md:h-4" />

      {!settingsEditing ? (
        <Button block theme="primary" onClick={() => setSettingsEditing(true)}>
          编辑
        </Button>
      ) : (
        <>
          <CellGroup title="编辑配置（保存后立即生效）" theme="card">
            <Form form={llmForm} labelAlign="left">
              <Form.FormItem label="模型" name="openai_model">
                <Input placeholder="例如 minimax-m2.5" />
              </Form.FormItem>
              <Form.FormItem label="Base URL" name="openai_base_url">
                <Input placeholder="例如 http://xxx/v1" />
              </Form.FormItem>
              <Form.FormItem label="超时(秒)" name="openai_timeout_seconds">
                <Input type="number" placeholder="例如 60" />
              </Form.FormItem>
              <Form.FormItem label="MaxTokens" name="openai_max_tokens">
                <Input type="number" placeholder="例如 8192" />
              </Form.FormItem>
              <Form.FormItem label="API Key" name="openai_api_key" help="留空表示不修改（不会清空已有 key）">
                <Input type="password" placeholder="sk-..." />
              </Form.FormItem>
            </Form>
          </CellGroup>
          <div className="h-3 md:h-4" />
          <div className="space-y-2">
            <Button
              block
              theme="primary"
              loading={opBusy}
              onClick={async () => {
                setOpBusy(true);
                try {
                  const v = llmForm.getFieldsValue(true) as unknown as LlmFormData;
                  const res = await apiPutLLMSettings({
                    openai_model: v.openai_model?.trim() || undefined,
                    openai_base_url: v.openai_base_url?.trim() || undefined,
                    openai_timeout_seconds: v.openai_timeout_seconds
                      ? Number(v.openai_timeout_seconds)
                      : undefined,
                    openai_max_tokens: v.openai_max_tokens
                      ? Number(v.openai_max_tokens)
                      : undefined,
                    openai_api_key: v.openai_api_key || undefined,
                  });
                  setLlmSettings(res.settings);
                  Toast.success({ message: "已保存" });
                  if (res.warnings?.length) {
                    Toast.warning({ message: res.warnings.join("；") });
                  }
                  setSettingsEditing(false);
                } catch (e) {
                  Toast.error({ message: (e as Error).message });
                } finally {
                  setOpBusy(false);
                }
              }}
            >
              保存
            </Button>
            <Button block variant="outline" onClick={() => setSettingsEditing(false)}>
              取消
            </Button>
          </div>
        </>
      )}
    </div>
  );

  const renderBasicConfig = (kind: "embedding" | "rerank") => {
    const cur = kind === "embedding" ? embeddingSettings : rerankSettings;
    const title = kind === "embedding" ? "Embedding" : "Rerank";
    const save = kind === "embedding" ? apiPutEmbeddingSettings : apiPutRerankSettings;
    const test = kind === "embedding" ? apiTestEmbeddingConnection : apiTestRerankConnection;
    const setCur = kind === "embedding" ? setEmbeddingSettings : setRerankSettings;

    return (
      <div className="p-3 md:p-4">
        {cur
          ? renderReadonlyModelCells(
              cur.model,
              cur.openai_base_url ?? "(默认)",
              cur.openai_timeout_seconds,
              cur.openai_max_tokens,
              cur.openai_api_key_configured,
            )
          : (
            <CellGroup title="当前生效配置" theme="card">
              <Cell title="加载中…" />
            </CellGroup>
          )}

        <div className="h-3 md:h-4" />
        <Button
          block
          variant="outline"
          loading={opBusy}
          onClick={async () => {
            setOpBusy(true);
            try {
              const resp = await test({
                openai_model: cur?.model,
                openai_base_url: cur?.openai_base_url ?? undefined,
              });
              if (resp.ok) Toast.success({ message: resp.message });
              else Toast.error({ message: resp.message });
            } catch (e) {
              Toast.error({ message: (e as Error).message });
            } finally {
              setOpBusy(false);
            }
          }}
        >
          测试连通性
        </Button>
        <div className="h-3 md:h-4" />

        {!settingsEditing ? (
          <Button block theme="primary" onClick={() => setSettingsEditing(true)}>
            编辑
          </Button>
        ) : (
          <>
            <CellGroup title={`编辑配置（保存后立即生效）`} theme="card">
              <Form form={basicForm} labelAlign="left">
                <Form.FormItem label="模型" name="model">
                  <Input placeholder={`例如 ${title} 模型名`} />
                </Form.FormItem>
                <Form.FormItem label="Base URL" name="openai_base_url">
                  <Input placeholder="例如 http://xxx/v1" />
                </Form.FormItem>
                <Form.FormItem label="超时(秒)" name="openai_timeout_seconds">
                  <Input type="number" placeholder="例如 60" />
                </Form.FormItem>
                <Form.FormItem label="MaxTokens" name="openai_max_tokens">
                  <Input type="number" placeholder="例如 8192" />
                </Form.FormItem>
                <Form.FormItem label="API Key" name="openai_api_key" help="留空表示不修改（不会清空已有 key）">
                  <Input type="password" placeholder="sk-..." />
                </Form.FormItem>
              </Form>
            </CellGroup>

            <div className="h-3 md:h-4" />

            <div className="space-y-2">
              <Button
                block
                theme="primary"
                loading={opBusy}
                onClick={async () => {
                  setOpBusy(true);
                  try {
                    const v = basicForm.getFieldsValue(true) as unknown as BasicFormData;
                    const res = await save({
                      model: v.model?.trim() || undefined,
                      openai_base_url: v.openai_base_url?.trim() || undefined,
                      openai_timeout_seconds: v.openai_timeout_seconds
                        ? Number(v.openai_timeout_seconds)
                        : undefined,
                      openai_max_tokens: v.openai_max_tokens
                        ? Number(v.openai_max_tokens)
                        : undefined,
                      openai_api_key: v.openai_api_key || undefined,
                    });
                    setCur(res.settings);
                    Toast.success({ message: "已保存" });
                    setSettingsEditing(false);
                  } catch (e) {
                    Toast.error({ message: (e as Error).message });
                  } finally {
                    setOpBusy(false);
                  }
                }}
              >
                保存
              </Button>
              <Button block variant="outline" onClick={() => setSettingsEditing(false)}>
                取消
              </Button>
            </div>
          </>
        )}
      </div>
    );
  };

  const renderSettings = () => {
    if (settingsRoute === "root") return renderSettingsRoot();
    if (settingsRoute === "model-config-gate") return renderModelConfigGate();
    if (settingsRoute === "model-config") return renderModelConfigMenu();
    if (settingsRoute === "llm") return renderLlmConfig();
    if (settingsRoute === "embedding") return renderBasicConfig("embedding");
    return renderBasicConfig("rerank");
  };

  return (
    <div className="mx-auto flex min-h-[100dvh] w-full flex-col bg-background md:max-w-3xl md:border-x">
      <Navbar title={navTitle} leftArrow={showBack} onLeftClick={onBack} />
      <div className="min-h-0 flex-1 pb-20">
        {tab === "home" ? renderHome() : null}
        {tab === "chat" ? renderChat() : null}
        {tab === "settings" ? renderSettings() : null}
        {status.error ? (
          <>
            <Divider />
            <div className="px-4 pb-6 text-xs text-destructive">
              <strong>错误：</strong> {status.error}
            </div>
          </>
        ) : null}
      </div>
      <div className="fixed inset-x-0 bottom-0 z-20 bg-background/95 backdrop-blur">
        <div className="mx-auto w-full md:max-w-3xl md:border-x">
          <TabBar value={tab} onChange={(v) => setTab(v as TabKey)}>
            {tabItems.map((item) => (
              <TabBarItem key={item.key} value={item.key} icon={item.icon}>
                {item.label}
              </TabBarItem>
            ))}
          </TabBar>
        </div>
      </div>
    </div>
  );
}
