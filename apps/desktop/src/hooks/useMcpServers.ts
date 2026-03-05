import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";

import type {
  McpConnectionTestResult,
  McpOperationLog,
  McpServer,
  McpTransport,
  SaveMcpServerResponse,
  SyncMcpConfigsResponse,
  ThreadProviderId,
} from "@/types";

export interface McpDraftInput {
  id?: string;
  name: string;
  transport: McpTransport;
  target: string;
  argsJson: string;
  headersJson: string;
  envJson: string;
  scopeProviders: ThreadProviderId[];
  enabled: boolean;
  version: string;
  secretHeaderName?: string;
  secretToken?: string;
  clearSecret: boolean;
}

export interface UseMcpServersResult {
  servers: McpServer[];
  logs: McpOperationLog[];
  loading: boolean;
  saving: boolean;
  testing: boolean;
  syncing: boolean;
  error: string | null;
  loadServers: () => Promise<void>;
  loadLogs: (limit?: number) => Promise<void>;
  saveServer: (draft: McpDraftInput) => Promise<SaveMcpServerResponse>;
  deleteServer: (id: string) => Promise<void>;
  toggleServerEnabled: (id: string, enabled: boolean) => Promise<void>;
  testConnection: (draft: McpDraftInput) => Promise<McpConnectionTestResult>;
  syncConfigs: (providerIds?: ThreadProviderId[]) => Promise<SyncMcpConfigsResponse>;
}

function upsertServerLocal(current: McpServer[], nextServer: McpServer): McpServer[] {
  const existingIndex = current.findIndex((item) => item.id === nextServer.id);
  if (existingIndex < 0) {
    return [...current, nextServer].sort((left, right) =>
      left.name.localeCompare(right.name),
    );
  }

  const next = [...current];
  next[existingIndex] = nextServer;
  return next;
}

function isThreadProviderId(value: string): value is ThreadProviderId {
  return value === "claude_code" || value === "codex" || value === "opencode";
}

function resolveProviderIdsForSync(value: readonly string[] | undefined): ThreadProviderId[] {
  if (!value || value.length === 0) {
    return ["claude_code", "codex", "opencode"];
  }
  const next = value.filter(isThreadProviderId);
  if (next.length === 0) {
    return ["claude_code", "codex", "opencode"];
  }
  return Array.from(new Set(next));
}

export function useMcpServers(): UseMcpServersResult {
  const [servers, setServers] = useState<McpServer[]>([]);
  const [logs, setLogs] = useState<McpOperationLog[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadServers = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<McpServer[]>("list_mcp_servers");
      setServers(data);
    } catch (loadError) {
      const message =
        loadError instanceof Error ? loadError.message : String(loadError);
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  const loadLogs = useCallback(async (limit?: number) => {
    try {
      const data = await invoke<McpOperationLog[]>("list_mcp_operation_logs", {
        limit: limit ?? 50,
      });
      setLogs(data);
    } catch (loadError) {
      const message =
        loadError instanceof Error ? loadError.message : String(loadError);
      setError(message);
    }
  }, []);

  const runAutoSync = useCallback(
    async (providerIds: ThreadProviderId[], successPrefix: string): Promise<void> => {
      setSyncing(true);
      try {
        const response = await invoke<SyncMcpConfigsResponse>("sync_mcp_configs", {
          request: {
            providerIds,
          },
        });
        await loadLogs(50);
        if (!response.success) {
          setError(
            `${successPrefix}，但自动同步失败：${response.message ?? "unknown error"}`,
          );
        }
      } catch (syncError) {
        const message =
          syncError instanceof Error ? syncError.message : String(syncError);
        setError(`${successPrefix}，但自动同步失败：${message}`);
      } finally {
        setSyncing(false);
      }
    },
    [loadLogs],
  );

  const saveServer = useCallback(
    async (draft: McpDraftInput): Promise<SaveMcpServerResponse> => {
      setSaving(true);
      setError(null);
      try {
        const previousServer = draft.id
          ? servers.find((item) => item.id === draft.id)
          : undefined;
        const response = await invoke<SaveMcpServerResponse>("save_mcp_server", {
          request: {
            id: draft.id,
            name: draft.name,
            transport: draft.transport,
            target: draft.target,
            argsJson: draft.argsJson,
            headersJson: draft.headersJson,
            envJson: draft.envJson,
            scopeProviders: draft.scopeProviders,
            enabled: draft.enabled,
            version: draft.version,
            secretHeaderName: draft.secretHeaderName,
            secretToken: draft.secretToken,
            clearSecret: draft.clearSecret,
          },
        });

        const nextServer = response.server ?? null;
        if (nextServer) {
          setServers((current) => upsertServerLocal(current, nextServer));
          await loadLogs(50);
          const syncProviderIds = resolveProviderIdsForSync([
            ...(previousServer?.scopeProviders ?? []),
            ...(nextServer.scopeProviders ?? []),
          ]);
          await runAutoSync(
            syncProviderIds,
            "保存成功",
          );
        }

        return response;
      } catch (saveError) {
        const message =
          saveError instanceof Error ? saveError.message : String(saveError);
        setError(message);
        throw new Error(message);
      } finally {
        setSaving(false);
      }
    },
    [loadLogs, runAutoSync, servers],
  );

  const deleteServer = useCallback(
    async (id: string): Promise<void> => {
      setError(null);
      try {
        const currentServer = servers.find((item) => item.id === id);
        await invoke("delete_mcp_server", {
          request: { id },
        });
        setServers((current) => current.filter((item) => item.id !== id));
        await loadLogs(50);
        await runAutoSync(
          resolveProviderIdsForSync(currentServer?.scopeProviders),
          "删除成功",
        );
      } catch (deleteError) {
        const message =
          deleteError instanceof Error ? deleteError.message : String(deleteError);
        setError(message);
        throw new Error(message);
      }
    },
    [loadLogs, runAutoSync, servers],
  );

  const toggleServerEnabled = useCallback(
    async (id: string, enabled: boolean): Promise<void> => {
      setError(null);
      try {
        const currentServer = servers.find((item) => item.id === id);
        await invoke("toggle_mcp_server_enabled", {
          request: { id, enabled },
        });
        setServers((current) =>
          current.map((server) =>
            server.id === id ? { ...server, enabled } : server,
          ),
        );
        await loadLogs(50);
        await runAutoSync(
          resolveProviderIdsForSync(currentServer?.scopeProviders),
          enabled ? "启用成功" : "停用成功",
        );
      } catch (toggleError) {
        const message =
          toggleError instanceof Error ? toggleError.message : String(toggleError);
        setError(message);
        throw new Error(message);
      }
    },
    [loadLogs, runAutoSync, servers],
  );

  const testConnection = useCallback(
    async (draft: McpDraftInput): Promise<McpConnectionTestResult> => {
      setTesting(true);
      setError(null);
      try {
        const response = await invoke<McpConnectionTestResult>(
          "test_mcp_server_connection",
          {
            request: {
              id: draft.id,
              transport: draft.transport,
              target: draft.target,
              argsJson: draft.argsJson,
              headersJson: draft.headersJson,
              envJson: draft.envJson,
              secretHeaderName: draft.secretHeaderName,
              secretToken: draft.secretToken,
            },
          },
        );

        if (draft.id) {
          await loadServers();
          await loadLogs(50);
        }

        return response;
      } catch (testError) {
        const message =
          testError instanceof Error ? testError.message : String(testError);
        setError(message);
        throw new Error(message);
      } finally {
        setTesting(false);
      }
    },
    [loadLogs, loadServers],
  );

  const syncConfigs = useCallback(
    async (providerIds?: ThreadProviderId[]): Promise<SyncMcpConfigsResponse> => {
      setSyncing(true);
      setError(null);
      try {
        const response = await invoke<SyncMcpConfigsResponse>("sync_mcp_configs", {
          request: {
            providerIds,
          },
        });
        await loadLogs(50);
        return response;
      } catch (syncError) {
        const message =
          syncError instanceof Error ? syncError.message : String(syncError);
        setError(message);
        throw new Error(message);
      } finally {
        setSyncing(false);
      }
    },
    [loadLogs],
  );

  useEffect(() => {
    void (async () => {
      await loadServers();
      await loadLogs(50);
    })();
  }, [loadLogs, loadServers]);

  return {
    servers,
    logs,
    loading,
    saving,
    testing,
    syncing,
    error,
    loadServers,
    loadLogs,
    saveServer,
    deleteServer,
    toggleServerEnabled,
    testConnection,
    syncConfigs,
  };
}
