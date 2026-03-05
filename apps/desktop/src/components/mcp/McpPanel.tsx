import {
  AlertTriangle,
  CheckCircle2,
  ChevronDown,
  Loader2,
  Plus,
  RefreshCw,
  Save,
  ServerCog,
  Settings2,
  Trash2,
  Wifi,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";

import { useMcpServers, type McpDraftInput } from "@/hooks/useMcpServers";
import type {
  McpConnectionTestResult,
  McpFieldError,
  McpServer,
  McpTransport,
  SyncMcpConfigsResponse,
  ThreadProviderId,
} from "@/types";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ProviderIcon } from "@/components/provider/ProviderIcon";
import { Switch } from "@/components/ui/switch";
import { JsonCodeEditor } from "@/components/ui/json-code-editor";
import { providerDisplayName } from "@/lib/provider";
import { cn } from "@/lib/utils";

const PROVIDERS: ThreadProviderId[] = ["claude_code", "codex", "opencode"];
const CONFIG_ERROR_FIELDS = new Set([
  "transport",
  "target",
  "argsJson",
  "headersJson",
  "envJson",
  "scopeProviders",
  "secretHeaderName",
  "version",
]);

interface McpConfigDocument {
  type?: unknown;
  transport?: unknown;
  command?: unknown;
  url?: unknown;
  target?: unknown;
  args?: unknown;
  headers?: unknown;
  http_headers?: unknown;
  env?: unknown;
  environment?: unknown;
  scopeProviders?: unknown;
  enabled?: unknown;
  version?: unknown;
  secretHeaderName?: unknown;
}

function isThreadProviderId(value: string): value is ThreadProviderId {
  return value === "claude_code" || value === "codex" || value === "opencode";
}

function isTransport(value: string): value is McpTransport {
  return value === "stdio" || value === "http" || value === "sse";
}

function createEmptyDraft(): McpDraftInput {
  return {
    id: undefined,
    name: "",
    transport: "stdio",
    target: "",
    argsJson: "[]",
    headersJson: "{}",
    envJson: "{}",
    scopeProviders: [...PROVIDERS],
    enabled: true,
    version: "1",
    secretHeaderName: "Authorization",
    secretToken: "",
    clearSecret: false,
  };
}

function parseProviderScope(value: unknown): ThreadProviderId[] {
  if (!Array.isArray(value)) {
    return [...PROVIDERS];
  }

  const next = value.filter((item): item is ThreadProviderId => {
    return typeof item === "string" && isThreadProviderId(item);
  });

  return Array.from(new Set(next));
}

function mapServerToDraft(server: McpServer): McpDraftInput {
  return {
    id: server.id,
    name: server.name,
    transport: server.transport,
    target: server.target,
    argsJson: server.argsJson,
    headersJson: server.headersJson,
    envJson: server.envJson,
    scopeProviders: parseProviderScope(server.scopeProviders),
    enabled: server.enabled,
    version: server.version || "1",
    secretHeaderName: server.secretHeaderName ?? "Authorization",
    secretToken: "",
    clearSecret: false,
  };
}

function toConfigJson(draft: McpDraftInput): string {
  const args = safeParseJsonArray(draft.argsJson);
  const headers = safeParseJsonObject(draft.headersJson);
  const env = safeParseJsonObject(draft.envJson);
  const document: Record<string, unknown> =
    draft.transport === "stdio"
      ? {
          type: "stdio",
          command: draft.target,
          ...(args.length > 0 ? { args } : {}),
          ...(Object.keys(env).length > 0 ? { env } : {}),
        }
      : {
          type: draft.transport,
          url: draft.target,
          ...(Object.keys(headers).length > 0 ? { headers } : {}),
        };

  return JSON.stringify(document, null, 2);
}

function safeParseJsonArray(raw: string): string[] {
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed
      .filter((item): item is string => typeof item === "string")
      .map((item) => item.trim())
      .filter((item) => item.length > 0);
  } catch {
    return [];
  }
}

function safeParseJsonObject(raw: string): Record<string, string> {
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return {};
    }

    const next: Record<string, string> = {};
    for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
      if (typeof value === "string") {
        next[key] = value;
      }
    }
    return next;
  } catch {
    return {};
  }
}

function parseStringMap(value: unknown, field: string): Record<string, string> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${field} 必须是对象`);
  }
  const next: Record<string, string> = {};
  for (const [key, entry] of Object.entries(value as Record<string, unknown>)) {
    if (typeof entry !== "string") {
      throw new Error(`${field}.${key} 必须是字符串`);
    }
    next[key] = entry;
  }
  return next;
}

function parseStringArray(value: unknown, field: string): string[] {
  if (!Array.isArray(value)) {
    throw new Error(`${field} 必须是字符串数组`);
  }
  const next: string[] = [];
  for (const item of value) {
    if (typeof item !== "string") {
      throw new Error(`${field} 必须是字符串数组`);
    }
    next.push(item);
  }
  return next;
}

function parseConfigJsonToDraft(base: McpDraftInput, raw: string): McpDraftInput {
  const trimmed = raw.trim();
  if (!trimmed) {
    throw new Error("配置 JSON 不能为空");
  }

  const parsed = JSON.parse(trimmed) as unknown;
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
    throw new Error("配置 JSON 顶层必须是对象");
  }

  const document = parsed as McpConfigDocument;
  const next: McpDraftInput = {
    ...base,
    secretToken: base.secretToken,
    clearSecret: base.clearSecret,
  };

  if (typeof document.type === "string") {
    const type = document.type.trim().toLowerCase();
    if (type === "local") {
      next.transport = "stdio";
    } else if (type === "remote") {
      next.transport = "sse";
    } else if (isTransport(type)) {
      next.transport = type;
    } else {
      throw new Error("type 仅支持 stdio/http/sse/local/remote");
    }
  }

  if (typeof document.transport === "string") {
    const transport = document.transport.trim().toLowerCase();
    if (!isTransport(transport)) {
      throw new Error("transport 仅支持 stdio/http/sse");
    }
    next.transport = transport;
  }

  if (typeof document.target === "string") {
    next.target = document.target;
  }

  let commandArray: string[] | null = null;
  if (Array.isArray(document.command)) {
    commandArray = parseStringArray(document.command, "command");
  } else if (typeof document.command === "string") {
    if (next.transport === "stdio") {
      next.target = document.command;
    } else if (!next.target.trim()) {
      next.target = document.command;
    }
  }

  if (typeof document.url === "string") {
    if (next.transport === "stdio") {
      if (!next.target.trim()) {
        next.target = document.url;
      }
    } else {
      next.target = document.url;
    }
  }

  if (document.args !== undefined) {
    next.argsJson = JSON.stringify(parseStringArray(document.args, "args"));
  } else if (commandArray && next.transport === "stdio") {
    next.argsJson = JSON.stringify(commandArray.slice(1));
  }

  if (next.transport === "stdio" && commandArray && commandArray.length > 0) {
    next.target = commandArray[0];
  }

  const headersValue =
    document.headers !== undefined ? document.headers : document.http_headers;
  if (headersValue !== undefined) {
    next.headersJson = JSON.stringify(parseStringMap(headersValue, "headers"));
  }

  const envValue = document.env !== undefined ? document.env : document.environment;
  if (envValue !== undefined) {
    next.envJson = JSON.stringify(parseStringMap(envValue, "env"));
  }

  if (document.scopeProviders !== undefined) {
    if (!Array.isArray(document.scopeProviders)) {
      throw new Error("scopeProviders 必须是数组");
    }
    const scope = document.scopeProviders
      .filter((item): item is ThreadProviderId => {
        return typeof item === "string" && isThreadProviderId(item);
      });
    next.scopeProviders = Array.from(new Set(scope));
  }

  if (document.enabled !== undefined) {
    if (typeof document.enabled !== "boolean") {
      throw new Error("enabled 必须是布尔值");
    }
    next.enabled = document.enabled;
  }

  if (document.version !== undefined) {
    if (typeof document.version !== "string") {
      throw new Error("version 必须是字符串");
    }
    next.version = document.version;
  }

  if (document.secretHeaderName !== undefined) {
    if (typeof document.secretHeaderName !== "string") {
      throw new Error("secretHeaderName 必须是字符串");
    }
    next.secretHeaderName = document.secretHeaderName;
  }

  return next;
}

function buildFieldErrorMap(fieldErrors: McpFieldError[]): Record<string, string> {
  const next: Record<string, string> = {};
  for (const fieldError of fieldErrors) {
    if (!next[fieldError.field]) {
      next[fieldError.field] = fieldError.message;
    }
  }
  return next;
}

function summarizeConfigFieldErrors(fieldErrors: Record<string, string>): string[] {
  const messages = Object.entries(fieldErrors)
    .filter(([field]) => CONFIG_ERROR_FIELDS.has(field))
    .map(([field, message]) => `${field}: ${message}`);
  return messages;
}

function summarizeAuditDetails(detailsJson: string): string {
  try {
    const parsed = JSON.parse(detailsJson) as { summary?: string };
    if (parsed.summary && parsed.summary.trim().length > 0) {
      return parsed.summary;
    }
  } catch {
    // keep raw fallback
  }
  return detailsJson;
}

function formatDateTime(value?: string | null): string {
  if (!value) {
    return "-";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

function formatTransport(transport: string): string {
  if (transport === "stdio") {
    return "stdio";
  }
  if (transport === "http") {
    return "http";
  }
  if (transport === "sse") {
    return "sse";
  }
  return transport;
}

export function McpPanel() {
  const {
    servers,
    logs,
    loading,
    saving,
    testing,
    syncing,
    error,
    saveServer,
    deleteServer,
    testConnection,
    syncConfigs,
    loadServers,
  } = useMcpServers();

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<McpDraftInput>(createEmptyDraft());
  const [configJson, setConfigJson] = useState<string>(toConfigJson(createEmptyDraft()));
  const [configJsonError, setConfigJsonError] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const [notice, setNotice] = useState<string | null>(null);
  const [noticeTone, setNoticeTone] = useState<"success" | "error">("success");
  const [testResult, setTestResult] = useState<McpConnectionTestResult | null>(null);
  const [syncResult, setSyncResult] = useState<SyncMcpConfigsResponse | null>(null);
  const [wizardOpen, setWizardOpen] = useState(false);
  const [wizardDraft, setWizardDraft] = useState<McpDraftInput>(createEmptyDraft());
  const [deleting, setDeleting] = useState(false);
  const [providerToggleKey, setProviderToggleKey] = useState<string | null>(null);
  const configEditorDarkMode =
    typeof document !== "undefined" && document.documentElement.classList.contains("dark");

  const selectedServer = useMemo(
    () => servers.find((item) => item.id === selectedId) ?? null,
    [selectedId, servers],
  );

  useEffect(() => {
    if (!selectedId) {
      return;
    }
    const next = servers.find((item) => item.id === selectedId);
    if (!next) {
      const empty = createEmptyDraft();
      setSelectedId(null);
      setDraft(empty);
      setConfigJson(toConfigJson(empty));
      return;
    }

    const mapped = mapServerToDraft(next);
    setDraft(mapped);
    setConfigJson(toConfigJson(mapped));
  }, [selectedId, servers]);

  const applyConfigJson = useCallback(
    (sourceDraft: McpDraftInput): McpDraftInput | null => {
      try {
        const parsed = parseConfigJsonToDraft(sourceDraft, configJson);
        setConfigJsonError(null);
        return parsed;
      } catch (parseError) {
        const message =
          parseError instanceof Error ? parseError.message : String(parseError);
        setConfigJsonError(message);
        setNotice(`配置 JSON 无法解析：${message}`);
        setNoticeTone("error");
        return null;
      }
    },
    [configJson],
  );

  const startCreateNew = useCallback(() => {
    const empty = createEmptyDraft();
    setSelectedId(null);
    setDraft(empty);
    setConfigJson(toConfigJson(empty));
    setConfigJsonError(null);
    setFieldErrors({});
    setNotice(null);
    setTestResult(null);
    setSyncResult(null);
  }, []);

  const handleSelectServer = useCallback((server: McpServer) => {
    const mapped = mapServerToDraft(server);
    setSelectedId(server.id);
    setDraft(mapped);
    setConfigJson(toConfigJson(mapped));
    setConfigJsonError(null);
    setFieldErrors({});
    setNotice(null);
    setTestResult(null);
    setSyncResult(null);
  }, []);

  const handleSave = useCallback(async () => {
    const parsedDraft = applyConfigJson(draft);
    if (!parsedDraft) {
      return;
    }

    const response = await saveServer(parsedDraft);
    if (!response.server || response.fieldErrors.length > 0) {
      setFieldErrors(buildFieldErrorMap(response.fieldErrors));
      setNotice(response.message ?? "保存失败，请修复字段错误。");
      setNoticeTone("error");
      return;
    }

    const next = mapServerToDraft(response.server);
    setSelectedId(response.server.id);
    setDraft(next);
    setConfigJson(toConfigJson(next));
    setConfigJsonError(null);
    setFieldErrors({});
    setNotice("保存成功，已自动同步到对应 provider 配置。若失败会在下方提示。");
    setNoticeTone("success");
  }, [applyConfigJson, draft, saveServer]);

  const handleTest = useCallback(async () => {
    const parsedDraft = applyConfigJson(draft);
    if (!parsedDraft) {
      return;
    }

    setDraft(parsedDraft);
    const result = await testConnection(parsedDraft);
    setTestResult(result);

    if (result.fieldErrors.length > 0) {
      setFieldErrors(buildFieldErrorMap(result.fieldErrors));
      setNotice("连接测试被字段校验阻止。");
      setNoticeTone("error");
      return;
    }

    setNotice(result.success ? "连接测试通过。" : "连接测试失败。请检查配置。")
    setNoticeTone(result.success ? "success" : "error");
  }, [applyConfigJson, draft, testConnection]);

  const handleDelete = useCallback(async () => {
    if (!selectedId) {
      return;
    }
    const confirmed = window.confirm("删除此 MCP 服务？");
    if (!confirmed) {
      return;
    }

    setDeleting(true);
    try {
      await deleteServer(selectedId);
      const empty = createEmptyDraft();
      setSelectedId(null);
      setDraft(empty);
      setConfigJson(toConfigJson(empty));
      setConfigJsonError(null);
      setFieldErrors({});
      setNotice("删除成功，已自动同步。若失败会显示错误。")
      setNoticeTone("success");
      setTestResult(null);
      setSyncResult(null);
    } finally {
      setDeleting(false);
    }
  }, [deleteServer, selectedId]);

  const openWizard = useCallback(() => {
    const parsedDraft = applyConfigJson(draft);
    if (!parsedDraft) {
      return;
    }

    setWizardDraft(parsedDraft);
    setWizardOpen(true);
    setFieldErrors({});
  }, [applyConfigJson, draft]);

  const updateWizardField = useCallback(
    <K extends keyof McpDraftInput>(field: K, value: McpDraftInput[K]) => {
      setWizardDraft((current) => ({ ...current, [field]: value }));
      setFieldErrors((current) => {
        if (!(field in current)) {
          return current;
        }
        const next = { ...current };
        delete next[field as string];
        return next;
      });
    },
    [],
  );

  const toggleWizardScopeProvider = useCallback(
    (provider: ThreadProviderId, checked: boolean) => {
      setWizardDraft((current) => {
        const nextScope = checked
          ? Array.from(new Set([...current.scopeProviders, provider]))
          : current.scopeProviders.filter((item) => item !== provider);
        return {
          ...current,
          scopeProviders: nextScope,
        };
      });
    },
    [],
  );

  const handleToggleProviderFromList = useCallback(
    async (server: McpServer, provider: ThreadProviderId) => {
      const currentScope = server.enabled ? parseProviderScope(server.scopeProviders) : [];
      const enabledForProvider = currentScope.includes(provider);
      const nextScope = enabledForProvider
        ? currentScope.filter((item) => item !== provider)
        : Array.from(new Set([...currentScope, provider]));
      const nextDraft: McpDraftInput = {
        ...mapServerToDraft(server),
        scopeProviders: nextScope,
        enabled: nextScope.length > 0,
      };

      setProviderToggleKey(`${server.id}:${provider}`);
      try {
        const response = await saveServer(nextDraft).catch((error) => {
          const message = error instanceof Error ? error.message : String(error);
          setNotice(message || "变更 provider 启用状态失败。");
          setNoticeTone("error");
          return null;
        });
        if (response) {
          if (!response.server || response.fieldErrors.length > 0) {
            setFieldErrors(buildFieldErrorMap(response.fieldErrors));
            setNotice(response.message ?? "变更 provider 启用状态失败。");
            setNoticeTone("error");
            return;
          }
          setNotice(
            `${enabledForProvider ? "停用" : "启用"} ${providerDisplayName(provider)} 成功，已自动同步。`,
          );
          setNoticeTone("success");
        }
      } finally {
        setProviderToggleKey(null);
      }
    },
    [saveServer],
  );

  const applyWizard = useCallback(() => {
    setDraft(wizardDraft);
    setConfigJson(toConfigJson(wizardDraft));
    setConfigJsonError(null);
    setWizardOpen(false);
    setNotice("已应用配置向导内容（未保存）。");
    setNoticeTone("success");
  }, [wizardDraft]);

  const handleManualSync = useCallback(async () => {
    const parsedDraft = applyConfigJson(draft);
    if (!parsedDraft) {
      return;
    }

    const result = await syncConfigs(parsedDraft.scopeProviders);
    setSyncResult(result);
    setNotice(
      result.success
        ? "手动同步完成。"
        : result.message ?? "手动同步失败。",
    );
    setNoticeTone(result.success ? "success" : "error");
    await loadServers();
  }, [applyConfigJson, draft, loadServers, syncConfigs]);

  const configFieldErrors = useMemo(
    () => summarizeConfigFieldErrors(fieldErrors),
    [fieldErrors],
  );

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <ServerCog className="h-4 w-4" />
          <p className="text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
            MCP Management
          </p>
        </div>
        <Badge variant="secondary" className="h-5 px-2 text-[10px]">
          {servers.length}
        </Badge>
      </div>

      <div className="grid gap-3 lg:grid-cols-[280px,1fr]">
        <div className="space-y-2 rounded-md border border-border/70 p-2">
          <div className="flex items-center gap-1.5">
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-7 flex-1 text-[11px]"
              onClick={startCreateNew}
            >
              <Plus className="mr-1.5 h-3 w-3" />
              New
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-7 px-2 text-[11px]"
              onClick={() => void loadServers()}
              disabled={loading}
            >
              {loading ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <RefreshCw className="h-3.5 w-3.5" />
              )}
            </Button>
          </div>

          <div className="max-h-80 space-y-1 overflow-y-auto">
            {servers.map((server) => {
              const selected = server.id === selectedId;
              const enabledProviders = server.enabled
                ? parseProviderScope(server.scopeProviders)
                : [];
              return (
                <div
                  key={server.id}
                  className={cn(
                    "rounded-md border border-border/60 px-2 py-2",
                    selected ? "border-primary/60 bg-primary/5" : "",
                  )}
                >
                  <div className="flex items-start justify-between gap-2">
                    <button
                      type="button"
                      className="min-w-0 flex-1 text-left"
                      onClick={() => handleSelectServer(server)}
                    >
                      <p className="truncate text-xs font-medium">{server.name}</p>
                      <p className="truncate text-[11px] text-muted-foreground">
                        {formatTransport(server.transport)} · {server.target}
                      </p>
                    </button>
                    <div className="flex items-center gap-1">
                      {PROVIDERS.map((provider) => {
                        const active = enabledProviders.includes(provider);
                        const key = `${server.id}:${provider}`;
                        const toggling = providerToggleKey === key;
                        return (
                          <button
                            key={provider}
                            type="button"
                            className={cn(
                              "inline-flex h-6 w-6 items-center justify-center rounded-md border transition-colors",
                              active
                                ? "border-primary/60 bg-primary/10"
                                : "border-border/70 bg-background/70 opacity-55 hover:opacity-90",
                            )}
                            disabled={saving || Boolean(providerToggleKey)}
                            onClick={() => void handleToggleProviderFromList(server, provider)}
                            title={`${providerDisplayName(provider)}: ${active ? "Enabled" : "Disabled"}`}
                          >
                            {toggling ? (
                              <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
                            ) : (
                              <ProviderIcon
                                providerId={provider}
                                className={cn("h-3.5 w-3.5", active ? "opacity-100" : "opacity-60")}
                              />
                            )}
                          </button>
                        );
                      })}
                    </div>
                  </div>
                </div>
              );
            })}

            {!loading && servers.length === 0 ? (
              <p className="px-1 py-1 text-[11px] text-muted-foreground">
                暂无 MCP 项。
              </p>
            ) : null}
          </div>
        </div>

        <div className="space-y-3 rounded-md border border-border/70 p-3">
          <label className="space-y-1">
            <span className="text-[11px] text-muted-foreground">Name</span>
            <input
              type="text"
              value={draft.name}
              onChange={(event) => {
                setDraft((current) => ({ ...current, name: event.target.value }));
                setFieldErrors((current) => {
                  if (!("name" in current)) {
                    return current;
                  }
                  const next = { ...current };
                  delete next.name;
                  return next;
                });
              }}
              className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs outline-none focus:border-primary"
              placeholder="Filesystem MCP"
            />
            {fieldErrors.name ? (
              <span className="text-[11px] text-destructive">{fieldErrors.name}</span>
            ) : null}
          </label>

          <div className="space-y-1">
            <div className="flex items-center justify-between">
              <span className="text-[11px] text-muted-foreground">Config JSON</span>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-7 px-2 text-[11px]"
                onClick={openWizard}
              >
                <Settings2 className="mr-1.5 h-3.5 w-3.5" />
                配置向导
              </Button>
            </div>
            <JsonCodeEditor
              value={configJson}
              darkMode={configEditorDarkMode}
              invalid={Boolean(configJsonError)}
              minHeight={256}
              onChange={(nextValue) => {
                setConfigJson(nextValue);
                setConfigJsonError(null);
                setFieldErrors({});
                setNotice(null);
                setTestResult(null);
              }}
            />
            <p className="text-[11px] text-muted-foreground">
              默认只维护 JSON。点击“配置向导”可用交互方式编辑，且会按 transport 自动显示/隐藏字段。
            </p>
            {configJsonError ? (
              <p className="text-[11px] text-destructive">{configJsonError}</p>
            ) : null}
            {configFieldErrors.length > 0 ? (
              <div className="space-y-0.5">
                {configFieldErrors.map((message) => (
                  <p key={message} className="text-[11px] text-destructive">
                    {message}
                  </p>
                ))}
              </div>
            ) : null}
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <Button
              type="button"
              variant="secondary"
              size="sm"
              className="h-8 px-2.5 text-xs"
              onClick={() => void handleSave()}
              disabled={saving}
            >
              {saving ? (
                <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
              ) : (
                <Save className="mr-1.5 h-3.5 w-3.5" />
              )}
              保存
            </Button>

            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-8 px-2.5 text-xs"
              onClick={() => void handleTest()}
              disabled={testing}
            >
              {testing ? (
                <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
              ) : (
                <Wifi className="mr-1.5 h-3.5 w-3.5" />
              )}
              测试连接
            </Button>

            {selectedId ? (
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-8 px-2.5 text-xs text-destructive"
                onClick={() => void handleDelete()}
                disabled={deleting}
              >
                {deleting ? (
                  <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Trash2 className="mr-1.5 h-3.5 w-3.5" />
                )}
                删除
              </Button>
            ) : null}
          </div>

          <p className="text-[11px] text-muted-foreground">
            保存与启停会自动同步到所选 provider；手动同步只用于修复或重放。
          </p>

          {testResult ? (
            <div
              className={cn(
                "rounded-md border px-2 py-1.5 text-[11px]",
                testResult.success
                  ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
                  : "border-destructive/30 bg-destructive/10 text-destructive",
              )}
            >
              <p className="inline-flex items-center gap-1">
                {testResult.success ? (
                  <CheckCircle2 className="h-3.5 w-3.5" />
                ) : (
                  <AlertTriangle className="h-3.5 w-3.5" />
                )}
                {testResult.success ? "Success" : "Failed"} · {testResult.durationMs}ms ·{" "}
                {formatDateTime(testResult.checkedAt)}
              </p>
              {testResult.errorSummary ? <p>{testResult.errorSummary}</p> : null}
            </div>
          ) : null}

          <details className="rounded-md border border-border/60 bg-muted/20 p-2">
            <summary className="inline-flex cursor-pointer items-center gap-1 text-[11px] text-muted-foreground">
              <ChevronDown className="h-3.5 w-3.5" />
              手动 Config Sync（高级）
            </summary>
            <div className="mt-2 space-y-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-8 px-2.5 text-xs"
                onClick={() => void handleManualSync()}
                disabled={syncing}
              >
                {syncing ? (
                  <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                ) : (
                  <RefreshCw className="mr-1.5 h-3.5 w-3.5" />
                )}
                立即同步
              </Button>
              {syncResult ? (
                <div className="space-y-1">
                  {syncResult.results.map((item) => (
                    <p
                      key={`${item.providerId}-${item.message ?? ""}`}
                      className={cn(
                        "text-[11px]",
                        item.success
                          ? "text-emerald-700 dark:text-emerald-300"
                          : "text-destructive",
                      )}
                    >
                      {item.providerId}: {item.message ?? (item.success ? "Synced" : "Failed")}
                    </p>
                  ))}
                </div>
              ) : null}
            </div>
          </details>

          {notice ? (
            <p
              className={cn(
                "text-[11px]",
                noticeTone === "success"
                  ? "text-emerald-700 dark:text-emerald-300"
                  : "text-destructive",
              )}
            >
              {notice}
            </p>
          ) : null}

          {error ? <p className="text-[11px] text-destructive">{error}</p> : null}
        </div>
      </div>

      <div className="space-y-1.5 rounded-md border border-border/70 p-2">
        <p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
          Audit Logs
        </p>
        <div className="max-h-48 space-y-1 overflow-y-auto">
          {logs.map((log) => (
            <div key={log.id} className="rounded border border-border/60 px-2 py-1.5">
              <p className="text-[11px] font-medium">
                {log.action} · {log.actor}
              </p>
              <p className="text-[11px] text-muted-foreground">
                {summarizeAuditDetails(log.detailsJson)}
              </p>
              <p className="text-[10px] text-muted-foreground/80">
                {formatDateTime(log.createdAt)}
              </p>
            </div>
          ))}
          {logs.length === 0 ? (
            <p className="text-[11px] text-muted-foreground">No audit events yet.</p>
          ) : null}
        </div>
      </div>

      {wizardOpen ? (
        <div
          className="fixed inset-0 z-[70] flex items-center justify-center bg-black/35 p-4"
          onClick={() => setWizardOpen(false)}
        >
          <div
            className="w-full max-w-2xl rounded-md border border-border bg-card p-3 shadow-xl"
            onClick={(event) => event.stopPropagation()}
          >
            <div className="mb-3 flex items-center justify-between">
              <p className="text-sm font-semibold">MCP 配置向导</p>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-7 px-2 text-xs"
                onClick={() => setWizardOpen(false)}
              >
                关闭
              </Button>
            </div>

            <div className="space-y-3">
              <div className="grid gap-2 sm:grid-cols-2">
                <label className="space-y-1">
                  <span className="text-[11px] text-muted-foreground">Transport</span>
                  <select
                    value={wizardDraft.transport}
                    onChange={(event) =>
                      updateWizardField(
                        "transport",
                        event.target.value as McpDraftInput["transport"],
                      )
                    }
                    className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs outline-none focus:border-primary"
                  >
                    <option value="stdio">stdio</option>
                    <option value="http">http</option>
                    <option value="sse">sse</option>
                  </select>
                </label>

                <label className="space-y-1">
                  <span className="text-[11px] text-muted-foreground">Version</span>
                  <input
                    type="text"
                    value={wizardDraft.version}
                    onChange={(event) => updateWizardField("version", event.target.value)}
                    className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs outline-none focus:border-primary"
                  />
                </label>
              </div>

              <label className="space-y-1">
                <span className="text-[11px] text-muted-foreground">
                  {wizardDraft.transport === "stdio" ? "Command" : "URL"}
                </span>
                <input
                  type="text"
                  value={wizardDraft.target}
                  onChange={(event) => updateWizardField("target", event.target.value)}
                  placeholder={wizardDraft.transport === "stdio" ? "npx" : "https://example.com/mcp"}
                  className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs outline-none focus:border-primary"
                />
              </label>

              {wizardDraft.transport === "stdio" ? (
                <>
                  <label className="space-y-1">
                    <span className="text-[11px] text-muted-foreground">Args JSON</span>
                    <textarea
                      value={wizardDraft.argsJson}
                      onChange={(event) => updateWizardField("argsJson", event.target.value)}
                      className="h-24 w-full rounded-md border border-input bg-background px-2 py-1.5 font-mono text-[11px] outline-none focus:border-primary"
                    />
                  </label>

                  <label className="space-y-1">
                    <span className="text-[11px] text-muted-foreground">Env JSON</span>
                    <textarea
                      value={wizardDraft.envJson}
                      onChange={(event) => updateWizardField("envJson", event.target.value)}
                      className="h-24 w-full rounded-md border border-input bg-background px-2 py-1.5 font-mono text-[11px] outline-none focus:border-primary"
                    />
                  </label>
                </>
              ) : (
                <>
                  <label className="space-y-1">
                    <span className="text-[11px] text-muted-foreground">Headers JSON</span>
                    <textarea
                      value={wizardDraft.headersJson}
                      onChange={(event) => updateWizardField("headersJson", event.target.value)}
                      className="h-24 w-full rounded-md border border-input bg-background px-2 py-1.5 font-mono text-[11px] outline-none focus:border-primary"
                    />
                  </label>

                  <div className="grid gap-2 sm:grid-cols-2">
                    <label className="space-y-1">
                      <span className="text-[11px] text-muted-foreground">Secret Header</span>
                      <input
                        type="text"
                        value={wizardDraft.secretHeaderName ?? ""}
                        onChange={(event) =>
                          updateWizardField(
                            "secretHeaderName",
                            event.target.value || undefined,
                          )
                        }
                        className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs outline-none focus:border-primary"
                      />
                    </label>
                    <label className="space-y-1">
                      <span className="text-[11px] text-muted-foreground">Secret Token</span>
                      <input
                        type="password"
                        value={wizardDraft.secretToken ?? ""}
                        onChange={(event) =>
                          updateWizardField("secretToken", event.target.value)
                        }
                        placeholder={selectedServer?.hasSecret ? "留空则保持" : "可选"}
                        className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs outline-none focus:border-primary"
                      />
                    </label>
                  </div>
                </>
              )}

              <div className="space-y-1">
                <p className="text-[11px] text-muted-foreground">Providers</p>
                <div className="flex flex-wrap gap-2">
                  {PROVIDERS.map((provider) => {
                    const active = wizardDraft.scopeProviders.includes(provider);
                    return (
                      <button
                        key={provider}
                        type="button"
                        onClick={() => toggleWizardScopeProvider(provider, !active)}
                        className={cn(
                          "rounded-full border px-2 py-1 text-[11px]",
                          active
                            ? "border-primary/60 bg-primary/10 text-foreground"
                            : "border-border/70 text-muted-foreground",
                        )}
                      >
                        {provider}
                      </button>
                    );
                  })}
                </div>
              </div>

              <label className="inline-flex items-center gap-1.5 text-[11px] text-muted-foreground">
                <Switch
                  checked={wizardDraft.enabled}
                  onCheckedChange={(checked) => updateWizardField("enabled", checked)}
                />
                Enabled
              </label>

              <div className="flex items-center justify-end gap-2">
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-8 px-3 text-xs"
                  onClick={() => setWizardOpen(false)}
                >
                  取消
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  className="h-8 px-3 text-xs"
                  onClick={applyWizard}
                >
                  应用到 JSON
                </Button>
              </div>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
