import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useState } from "react";

import DockAgentPanel from "@/components/dock-agent/DockAgentPanel";
import { ThreadHeader } from "@/components/header/ThreadHeader";
import { Sidebar } from "@/components/sidebar/Sidebar";
import { EmbeddedTerminal } from "@/components/terminal/EmbeddedTerminal";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { useSidebar } from "@/hooks/useSidebar";
import { useThreads } from "@/hooks/useThreads";
import { useWindowDrag } from "@/hooks/useWindowDrag";
import {
  buildIdeContextEnv,
  DEFAULT_OPEN_TARGET_STORAGE_KEY,
  IDE_CONTEXT_BY_THREAD_STORAGE_KEY,
  normalizeDefaultOpenTarget,
  parseIdeContextByThread,
  parseProjectOpenUsageMap,
  PROJECT_OPEN_USAGE_STORAGE_KEY,
  resolveQuickOpenTargetId,
  resolveDefaultOpenTarget,
  serializeIdeContextByThread,
  serializeProjectOpenUsageMap,
  sortTargetsByProjectUsage,
  setThreadIdeContextEnabled,
  updateProjectOpenUsage,
} from "@/lib/developerActions";
import { isSupportedProvider, providerDisplayName } from "@/lib/provider";
import { isAutomaticModeThread, resolveSelectedThreadKey, threadKey } from "@/lib/thread";
import { cn } from "@/lib/utils";
import type {
  AgentRuntimeSettings,
  AgentSupplier,
  AppWorkspaceMode,
  AppTheme,
  AppSkin,
  OpenTargetId,
  OpenTargetStatus,
  ProjectGitBranchInfo,
  ProviderProfileMap,
  SyncSophonAccountResult,
  TerminalTheme,
  ThreadProviderId,
} from "@/types";

const APP_THEME_KEY = "agentdock.desktop.app_theme";
const APP_SKIN_KEY = "agentdock.desktop.app_skin";
const AGENT_RUNTIME_SETTINGS_KEY = "agentdock.desktop.agent_runtime_settings";
const APP_WORKSPACE_MODE_KEY = "agentdock.desktop.workspace_mode";
const MAIN_VIEW_KEY = "agentdock.desktop.main_view";
const LEGACY_AGENT_PROFILE_SETTINGS_KEY = "agentdock.desktop.agent_profile_settings";
const LEGACY_ACTIVE_PROVIDER_KEY = "agentdock.desktop.active_provider";
const LEGACY_ACTIVE_PROFILE_KEY = "agentdock.desktop.active_profile";
const OFFICIAL_SUPPLIER_ID = "official-default";
const PROVIDER_IDS: ThreadProviderId[] = ["claude_code", "codex", "opencode", "sophon"];
const GIT_BRANCH_POLL_INTERVAL_MS = 8_000;

type MainView = "terminal" | "dock_agent";

interface OpenProjectWithTargetResponse {
  launched: boolean;
  targetId: string;
  command: string;
}

interface SyncSophonAccountRequest {
  baseUrl?: string | null;
  apiKey?: string | null;
  configJson?: string | null;
}

function readStoredDefaultOpenTarget(): OpenTargetId {
  if (typeof window === "undefined") {
    return normalizeDefaultOpenTarget(undefined);
  }
  return normalizeDefaultOpenTarget(
    window.localStorage.getItem(DEFAULT_OPEN_TARGET_STORAGE_KEY),
  );
}

function readStoredIdeContextByThread(): Record<string, boolean> {
  if (typeof window === "undefined") {
    return {};
  }
  return parseIdeContextByThread(
    window.localStorage.getItem(IDE_CONTEXT_BY_THREAD_STORAGE_KEY),
  );
}

function readStoredProjectOpenUsage(): ReturnType<typeof parseProjectOpenUsageMap> {
  if (typeof window === "undefined") {
    return {};
  }
  return parseProjectOpenUsageMap(
    window.localStorage.getItem(PROJECT_OPEN_USAGE_STORAGE_KEY),
  );
}

function readStoredAppTheme(): AppTheme {
  if (typeof window === "undefined") {
    return "light";
  }
  const raw = window.localStorage.getItem(APP_THEME_KEY);
  return raw === "dark" || raw === "system" ? raw : "light";
}

function readStoredAppSkin(): AppSkin {
  if (typeof window === "undefined") {
    return "default";
  }
  const raw = window.localStorage.getItem(APP_SKIN_KEY);
  return raw === "brutalism" ? "brutalism" : "default";
}

function readStoredWorkspaceMode(): AppWorkspaceMode {
  if (typeof window === "undefined") {
    return "manual";
  }
  const raw = window.localStorage.getItem(APP_WORKSPACE_MODE_KEY);
  return raw === "automatic" ? "automatic" : "manual";
}

function readStoredMainView(): MainView {
  if (typeof window === "undefined") {
    return "terminal";
  }
  return window.localStorage.getItem(MAIN_VIEW_KEY) === "dock_agent"
    ? "dock_agent"
    : "terminal";
}

function readSystemTheme(): TerminalTheme {
  if (typeof window === "undefined") {
    return "light";
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function normalizeProfileName(value: string | null | undefined): string {
  const trimmed = value?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : "default";
}

function normalizeOptionalText(value: string | null | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : undefined;
}

function createOfficialSupplier(
  providerId: ThreadProviderId,
  profileName = "default",
): AgentSupplier {
  return {
    id: OFFICIAL_SUPPLIER_ID,
    kind: "official",
    name: `Official ${providerDisplayName(providerId)}`,
    profileName: normalizeProfileName(profileName),
    updatedAt: Date.now(),
  };
}

function defaultAgentRuntimeSettings(): AgentRuntimeSettings {
  return {
    activeProviderId: "claude_code",
    activeSupplierIds: {
      claude_code: OFFICIAL_SUPPLIER_ID,
      codex: OFFICIAL_SUPPLIER_ID,
      opencode: OFFICIAL_SUPPLIER_ID,
      sophon: OFFICIAL_SUPPLIER_ID,
    },
    suppliersByProvider: {
      claude_code: [createOfficialSupplier("claude_code")],
      codex: [createOfficialSupplier("codex")],
      opencode: [createOfficialSupplier("opencode")],
      sophon: [createOfficialSupplier("sophon")],
    },
  };
}

function cloneAgentRuntimeSettings(settings: AgentRuntimeSettings): AgentRuntimeSettings {
  return JSON.parse(JSON.stringify(settings)) as AgentRuntimeSettings;
}

function normalizeAgentRuntimeSettings(input: AgentRuntimeSettings): AgentRuntimeSettings {
  const defaults = defaultAgentRuntimeSettings();
  const normalized: AgentRuntimeSettings = {
    activeProviderId: isSupportedProvider(input.activeProviderId)
      ? input.activeProviderId
      : defaults.activeProviderId,
    activeSupplierIds: {
      claude_code: OFFICIAL_SUPPLIER_ID,
      codex: OFFICIAL_SUPPLIER_ID,
      opencode: OFFICIAL_SUPPLIER_ID,
      sophon: OFFICIAL_SUPPLIER_ID,
    },
    suppliersByProvider: {
      claude_code: [],
      codex: [],
      opencode: [],
      sophon: [],
    },
  };

  for (const providerId of PROVIDER_IDS) {
    const rawList = input.suppliersByProvider?.[providerId] ?? [];
    const customSuppliers: AgentSupplier[] = [];
    let officialFromInput: AgentSupplier | null = null;
    const seenCustomIds = new Set<string>();

    for (const item of rawList) {
      if (!item || typeof item !== "object") {
        continue;
      }

      const kind = item.kind === "custom" ? "custom" : "official";
      const profileName = normalizeProfileName(item.profileName);
      const note = normalizeOptionalText(item.note);
      const baseUrl = normalizeOptionalText(item.baseUrl);
      const apiKey = normalizeOptionalText(item.apiKey);
      const configJson = normalizeOptionalText(item.configJson);
      const updatedAt = typeof item.updatedAt === "number" ? item.updatedAt : Date.now();

      if (kind === "official") {
        officialFromInput = {
          id: OFFICIAL_SUPPLIER_ID,
          kind: "official",
          name: normalizeOptionalText(item.name) ?? `Official ${providerDisplayName(providerId)}`,
          note,
          profileName,
          baseUrl,
          apiKey,
          configJson,
          updatedAt,
        };
        continue;
      }

      const requestedId = normalizeOptionalText(item.id) ?? `custom-${Date.now()}-${customSuppliers.length}`;
      let nextId = requestedId;
      while (seenCustomIds.has(nextId) || nextId === OFFICIAL_SUPPLIER_ID) {
        nextId = `${requestedId}-${customSuppliers.length + 1}`;
      }
      seenCustomIds.add(nextId);

      customSuppliers.push({
        id: nextId,
        kind: "custom",
        name: normalizeOptionalText(item.name) ?? "Custom Supplier",
        note,
        profileName,
        baseUrl,
        apiKey,
        configJson,
        updatedAt,
      });
    }

    const official =
      officialFromInput ??
      createOfficialSupplier(providerId, defaults.suppliersByProvider[providerId][0].profileName);
    normalized.suppliersByProvider[providerId] = [official, ...customSuppliers];

    const requestedActiveId = input.activeSupplierIds?.[providerId];
    const hasRequestedActive = normalized.suppliersByProvider[providerId].some(
      (supplier) => supplier.id === requestedActiveId,
    );
    normalized.activeSupplierIds[providerId] = hasRequestedActive
      ? (requestedActiveId as string)
      : OFFICIAL_SUPPLIER_ID;
  }

  return normalized;
}

function migrateLegacyProfileSelection(): AgentRuntimeSettings {
  const defaults = defaultAgentRuntimeSettings();
  if (typeof window === "undefined") {
    return defaults;
  }

  const selectionRaw = window.localStorage.getItem(LEGACY_AGENT_PROFILE_SETTINGS_KEY);
  if (selectionRaw) {
    try {
      const parsed = JSON.parse(selectionRaw) as {
        activeProviderId?: string;
        profiles?: Partial<Record<ThreadProviderId, string>>;
      };
      const activeProviderId = isSupportedProvider(parsed.activeProviderId ?? "")
        ? (parsed.activeProviderId as ThreadProviderId)
        : "claude_code";
      const next = cloneAgentRuntimeSettings(defaults);
      next.activeProviderId = activeProviderId;
      for (const providerId of PROVIDER_IDS) {
        next.suppliersByProvider[providerId][0].profileName = normalizeProfileName(
          parsed.profiles?.[providerId],
        );
      }
      return next;
    } catch {
      // Fallback to older storage schema.
    }
  }

  const legacyProviderRaw = window.localStorage.getItem(LEGACY_ACTIVE_PROVIDER_KEY);
  const legacyProviderId = isSupportedProvider(legacyProviderRaw ?? "")
    ? (legacyProviderRaw as ThreadProviderId)
    : "claude_code";
  const legacyProfile = normalizeProfileName(window.localStorage.getItem(LEGACY_ACTIVE_PROFILE_KEY));

  const fallback = cloneAgentRuntimeSettings(defaults);
  fallback.activeProviderId = legacyProviderId;
  for (const providerId of PROVIDER_IDS) {
    fallback.suppliersByProvider[providerId][0].profileName = legacyProfile;
  }
  return fallback;
}

function readStoredAgentRuntimeSettings(): AgentRuntimeSettings {
  if (typeof window === "undefined") {
    return defaultAgentRuntimeSettings();
  }

  const raw = window.localStorage.getItem(AGENT_RUNTIME_SETTINGS_KEY);
  if (!raw) {
    return migrateLegacyProfileSelection();
  }

  try {
    const parsed = JSON.parse(raw) as AgentRuntimeSettings;
    return normalizeAgentRuntimeSettings(parsed);
  } catch {
    return migrateLegacyProfileSelection();
  }
}

function resolveActiveSupplier(
  settings: AgentRuntimeSettings,
  providerId: ThreadProviderId,
): AgentSupplier {
  const suppliers = settings.suppliersByProvider[providerId] ?? [];
  const activeId = settings.activeSupplierIds[providerId];
  return (
    suppliers.find((supplier) => supplier.id === activeId) ??
    suppliers[0] ??
    createOfficialSupplier(providerId)
  );
}

function supplierSignature(supplier: AgentSupplier): string {
  return JSON.stringify({
    id: supplier.id,
    profileName: normalizeProfileName(supplier.profileName),
    baseUrl: normalizeOptionalText(supplier.baseUrl) ?? null,
    apiKey: normalizeOptionalText(supplier.apiKey) ?? null,
    configJson: normalizeOptionalText(supplier.configJson) ?? null,
  });
}

function hasSophonSupplierChanged(
  current: AgentRuntimeSettings,
  next: AgentRuntimeSettings,
): boolean {
  return (
    supplierSignature(resolveActiveSupplier(current, "sophon")) !==
    supplierSignature(resolveActiveSupplier(next, "sophon"))
  );
}

function buildSophonSyncRequest(settings: AgentRuntimeSettings): SyncSophonAccountRequest {
  const supplier = resolveActiveSupplier(settings, "sophon");
  return {
    baseUrl: normalizeOptionalText(supplier.baseUrl) ?? null,
    apiKey: normalizeOptionalText(supplier.apiKey) ?? null,
    configJson: normalizeOptionalText(supplier.configJson) ?? null,
  };
}

function deriveProviderProfiles(settings: AgentRuntimeSettings): ProviderProfileMap {
  return {
    claude_code: resolveActiveSupplier(settings, "claude_code").profileName,
    codex: resolveActiveSupplier(settings, "codex").profileName,
    opencode: resolveActiveSupplier(settings, "opencode").profileName,
    sophon: resolveActiveSupplier(settings, "sophon").profileName,
  };
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function normalizeStringValue(raw: unknown): string | undefined {
  if (typeof raw === "string") {
    const trimmed = raw.trim();
    return trimmed.length > 0 ? trimmed : undefined;
  }
  if (typeof raw === "number" || typeof raw === "boolean") {
    return String(raw);
  }
  return undefined;
}

function parseConfigJsonObject(configJson?: string): Record<string, unknown> | null {
  if (!configJson?.trim()) {
    return null;
  }

  try {
    const parsed = JSON.parse(configJson) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return null;
    }
    return parsed as Record<string, unknown>;
  } catch {
    return null;
  }
}

function extractEnvEntries(envValue: unknown): Record<string, string> {
  const envRecord = asRecord(envValue);
  if (!envRecord) {
    return {};
  }

  const result: Record<string, string> = {};
  for (const [key, rawValue] of Object.entries(envRecord)) {
    const envKey = key.trim();
    if (!envKey) {
      continue;
    }
    const normalizedValue = normalizeStringValue(rawValue);
    if (!normalizedValue) {
      continue;
    }
    result[envKey] = normalizedValue;
  }
  return result;
}

function firstNonEmptyString(candidates: unknown[]): string | undefined {
  for (const candidate of candidates) {
    const normalized = normalizeStringValue(candidate);
    if (normalized) {
      return normalized;
    }
  }
  return undefined;
}

function parseTomlScalar(configText: unknown, key: string): string | undefined {
  if (typeof configText !== "string") {
    return undefined;
  }

  for (const line of configText.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) {
      continue;
    }
    const equalsIndex = trimmed.indexOf("=");
    if (equalsIndex < 1) {
      continue;
    }
    const rawKey = trimmed.slice(0, equalsIndex).trim();
    if (rawKey !== key) {
      continue;
    }
    const rawValue = trimmed.slice(equalsIndex + 1).trim();
    if (!rawValue) {
      return undefined;
    }
    const unquoted = rawValue.replace(/^["']/, "").replace(/["']$/, "");
    const normalized = unquoted.trim();
    return normalized.length > 0 ? normalized : undefined;
  }

  return undefined;
}

function extractEnvFromConfigJson(
  providerId: ThreadProviderId,
  configJson?: string,
): Record<string, string> {
  const parsed = parseConfigJsonObject(configJson);
  if (!parsed) {
    return {};
  }

  const envOverrides = extractEnvEntries(parsed.env);
  const parsedEnvRecord = asRecord(parsed.env);
  const derived: Record<string, string> = {};

  if (providerId === "claude_code") {
    const apiKey = firstNonEmptyString([
      parsedEnvRecord?.ANTHROPIC_AUTH_TOKEN,
      parsedEnvRecord?.ANTHROPIC_API_KEY,
      parsed.apiKey,
      parsed.api_key,
    ]);
    const baseUrl = firstNonEmptyString([
      parsedEnvRecord?.ANTHROPIC_BASE_URL,
      parsed.baseURL,
      parsed.baseUrl,
      parsed.base_url,
      parsed.apiEndpoint,
    ]);

    if (apiKey) {
      derived.ANTHROPIC_AUTH_TOKEN = apiKey;
    }
    if (baseUrl) {
      derived.ANTHROPIC_BASE_URL = baseUrl;
    }
    return {
      ...derived,
      ...envOverrides,
    };
  }

  if (providerId === "codex") {
    const authRecord = asRecord(parsed.auth);
    const configRecord = asRecord(parsed.config);
    const apiKey = firstNonEmptyString([
      authRecord?.OPENAI_API_KEY,
      parsedEnvRecord?.OPENAI_API_KEY,
      parsedEnvRecord?.CODEX_API_KEY,
      configRecord?.apiKey,
      configRecord?.api_key,
      parsed.apiKey,
      parsed.api_key,
    ]);
    const baseUrl = firstNonEmptyString([
      parsedEnvRecord?.OPENAI_BASE_URL,
      parsed.baseURL,
      parsed.baseUrl,
      parsed.base_url,
      configRecord?.baseURL,
      configRecord?.baseUrl,
      configRecord?.base_url,
      parseTomlScalar(parsed.config, "base_url"),
    ]);

    if (apiKey) {
      derived.OPENAI_API_KEY = apiKey;
    }
    if (baseUrl) {
      derived.OPENAI_BASE_URL = baseUrl;
    }
    return {
      ...derived,
      ...envOverrides,
    };
  }

  if (providerId === "sophon") {
    return envOverrides;
  }

  const opencodeConfig = asRecord(parsed.settingsConfig) ?? parsed;
  const opencodeOptions = asRecord(opencodeConfig.options);
  const apiKey = firstNonEmptyString([
    opencodeOptions?.apiKey,
    opencodeOptions?.api_key,
    opencodeConfig.apiKey,
    opencodeConfig.api_key,
    parsedEnvRecord?.OPENCODE_API_KEY,
    parsedEnvRecord?.OPENAI_API_KEY,
  ]);
  const baseUrl = firstNonEmptyString([
    opencodeOptions?.baseURL,
    opencodeOptions?.baseUrl,
    opencodeOptions?.base_url,
    opencodeConfig.baseURL,
    opencodeConfig.baseUrl,
    opencodeConfig.base_url,
    parsedEnvRecord?.OPENCODE_BASE_URL,
    parsedEnvRecord?.OPENAI_BASE_URL,
  ]);

  if (apiKey) {
    derived.OPENCODE_API_KEY = apiKey;
  }
  if (baseUrl) {
    derived.OPENCODE_BASE_URL = baseUrl;
  }

  return {
    ...derived,
    ...envOverrides,
  };
}

function extractSophonDefaultProvider(configJson?: string): string | undefined {
  const parsed = parseConfigJsonObject(configJson);
  const settings = asRecord(parsed?.settings);
  return normalizeOptionalText(
    typeof settings?.defaultProvider === "string"
      ? settings.defaultProvider
      : typeof parsed?.defaultProvider === "string"
        ? parsed.defaultProvider
        : typeof parsed?.provider === "string"
          ? parsed.provider
          : undefined,
  );
}

function providerCredentialEnv(
  providerId: ThreadProviderId,
  supplier: AgentSupplier,
): Record<string, string> {
  const env: Record<string, string> = {};
  const apiKey = normalizeOptionalText(supplier.apiKey);
  const baseUrl = normalizeOptionalText(supplier.baseUrl);

  if (providerId === "claude_code") {
    if (apiKey) {
      env.ANTHROPIC_AUTH_TOKEN = apiKey;
    }
    if (baseUrl) {
      env.ANTHROPIC_BASE_URL = baseUrl;
    }
    return env;
  }

  if (providerId === "codex") {
    if (apiKey) {
      env.OPENAI_API_KEY = apiKey;
    }
    if (baseUrl) {
      env.OPENAI_BASE_URL = baseUrl;
    }
    return env;
  }

  if (providerId === "sophon") {
    const defaultProvider = extractSophonDefaultProvider(supplier.configJson);
    if (!apiKey || !defaultProvider) {
      return env;
    }

    if (defaultProvider === "zai") {
      env.ZAI_API_KEY = apiKey;
      return env;
    }
    if (defaultProvider === "openai") {
      env.OPENAI_API_KEY = apiKey;
      return env;
    }
    if (defaultProvider === "anthropic") {
      env.ANTHROPIC_API_KEY = apiKey;
      return env;
    }
    if (defaultProvider === "google") {
      env.GOOGLE_GENERATIVE_AI_API_KEY = apiKey;
      return env;
    }
    if (defaultProvider === "xai") {
      env.XAI_API_KEY = apiKey;
      return env;
    }
    if (defaultProvider === "openrouter") {
      env.OPENROUTER_API_KEY = apiKey;
      return env;
    }

    return env;
  }

  if (apiKey) {
    env.OPENCODE_API_KEY = apiKey;
  }
  if (baseUrl) {
    env.OPENCODE_BASE_URL = baseUrl;
  }
  return env;
}

function resolveLaunchEnvForProvider(
  settings: AgentRuntimeSettings,
  providerId: ThreadProviderId,
): Record<string, string> | undefined {
  const supplier = resolveActiveSupplier(settings, providerId);
  const credentialEnv = providerCredentialEnv(providerId, supplier);
  const configEnv = extractEnvFromConfigJson(providerId, supplier.configJson);
  const merged = {
    ...credentialEnv,
    ...configEnv,
  };
  return Object.keys(merged).length > 0 ? merged : undefined;
}

function validateAgentRuntimeSettings(settings: AgentRuntimeSettings): string | null {
  for (const providerId of PROVIDER_IDS) {
    const suppliers = settings.suppliersByProvider[providerId] ?? [];
    if (suppliers.length === 0) {
      return `${providerDisplayName(providerId)} requires at least one supplier.`;
    }

    const activeId = settings.activeSupplierIds[providerId];
    if (!suppliers.some((supplier) => supplier.id === activeId)) {
      return `${providerDisplayName(providerId)} active supplier is invalid.`;
    }

    for (const supplier of suppliers) {
      const supplierName = supplier.name.trim();
      if (!supplierName) {
        return `${providerDisplayName(providerId)} has a supplier with empty name.`;
      }

      if (!supplier.profileName.trim()) {
        return `${providerDisplayName(providerId)} supplier \"${supplierName}\" requires a profile name.`;
      }

      if (!supplier.configJson?.trim()) {
        continue;
      }

      try {
        const parsed = parseConfigJsonObject(supplier.configJson);
        if (!parsed) {
          return `${providerDisplayName(providerId)} supplier \"${supplierName}\" config JSON must be an object.`;
        }
        if (
          parsed.env !== undefined &&
          (typeof parsed.env !== "object" || Array.isArray(parsed.env) || parsed.env === null)
        ) {
          return `${providerDisplayName(providerId)} supplier \"${supplierName}\" config JSON field \"env\" must be an object when provided.`;
        }
      } catch {
        return `${providerDisplayName(providerId)} supplier \"${supplierName}\" has invalid config JSON.`;
      }
    }
  }

  return null;
}

function App() {
  const [appTheme, setAppTheme] = useState<AppTheme>(readStoredAppTheme);
  const [appSkin, setAppSkin] = useState<AppSkin>(readStoredAppSkin);
  const [workspaceMode, setWorkspaceMode] = useState<AppWorkspaceMode>(readStoredWorkspaceMode);
  const [agentRuntimeSettings, setAgentRuntimeSettings] = useState<AgentRuntimeSettings>(
    readStoredAgentRuntimeSettings,
  );
  const [mainView, setMainView] = useState<MainView>(readStoredMainView);
  const [systemTheme, setSystemTheme] = useState<TerminalTheme>(readSystemTheme);
  const resolvedTheme: TerminalTheme =
    appTheme === "system" ? systemTheme : appTheme;

  const activeProviderId = agentRuntimeSettings.activeProviderId;
  const providerProfiles = useMemo(
    () => deriveProviderProfiles(agentRuntimeSettings),
    [agentRuntimeSettings],
  );
  const activeProfileName = providerProfiles[activeProviderId] ?? "default";

  const {
    sidebarCollapsed,
    isResizingSidebar,
    layoutGridStyle,
    layoutRef,
    handleSidebarResizeStart,
    toggleSidebar,
  } = useSidebar();

  const {
    threads,
    selectedThreadKey,
    selectedThread,
    folderGroups,
    selectedFolderKey,
    loadingThreads,
    error,
    creatingThreadFolderKey,
    newThreadLaunch,
    newThreadBindingStatus,
    setError,
    loadThreads,
    handleSelectThread,
    handleCreateThreadInFolder,
    handleNewThreadLaunchSettled,
    handleEmbeddedTerminalSessionExit,
  } = useThreads();

  const [openTargets, setOpenTargets] = useState<OpenTargetStatus[]>([]);
  const [loadingOpenTargets, setLoadingOpenTargets] = useState(false);
  const [openTargetsLoadError, setOpenTargetsLoadError] = useState<string | null>(null);
  const [defaultOpenTargetId, setDefaultOpenTargetId] = useState<OpenTargetId>(
    readStoredDefaultOpenTarget,
  );
  const [openingTargetId, setOpeningTargetId] = useState<OpenTargetId | null>(null);
  const [openTargetActionError, setOpenTargetActionError] = useState<string | null>(null);
  const [ideContextByThread, setIdeContextByThread] = useState<Record<string, boolean>>(
    readStoredIdeContextByThread,
  );
  const [projectOpenUsage, setProjectOpenUsage] = useState(readStoredProjectOpenUsage);
  const [gitBranchInfo, setGitBranchInfo] = useState<ProjectGitBranchInfo | null>(null);
  const [gitBranchLoading, setGitBranchLoading] = useState(false);
  const [sophonWorkspacePath, setSophonWorkspacePath] = useState<string | null>(null);
  const [sophonWorkspacePathError, setSophonWorkspacePathError] = useState<string | null>(null);
  const automaticModeThreads = useMemo(
    () => threads.filter((thread) => isAutomaticModeThread(thread)),
    [threads],
  );
  const visibleSelectedThread = useMemo(() => {
    if (workspaceMode === "manual") {
      return selectedThread;
    }
    return automaticModeThreads.find((thread) => threadKey(thread) === selectedThreadKey) ?? null;
  }, [automaticModeThreads, selectedThread, selectedThreadKey, workspaceMode]);
  const automaticModeWorkspacePath =
    sophonWorkspacePath ?? automaticModeThreads[0]?.projectPath ?? null;

  const activeHeaderProjectPath = newThreadLaunch
    ? newThreadLaunch.projectPath
    : workspaceMode === "automatic"
      ? (visibleSelectedThread?.projectPath ?? automaticModeWorkspacePath)
      : (visibleSelectedThread?.projectPath ?? null);
  const normalizedActiveProjectPath = useMemo(() => {
    const trimmed = activeHeaderProjectPath?.trim();
    if (!trimmed || trimmed === "-") {
      return null;
    }
    return trimmed;
  }, [activeHeaderProjectPath]);

  const selectedThreadStorageKey = useMemo(() => {
    if (!visibleSelectedThread) {
      return null;
    }
    return threadKey(visibleSelectedThread);
  }, [visibleSelectedThread]);
  const visibleSelectedThreadKey = useMemo(() => {
    if (!visibleSelectedThread) {
      return null;
    }
    return threadKey(visibleSelectedThread);
  }, [visibleSelectedThread]);

  const ideContextToggleDisabled = !selectedThreadStorageKey || newThreadLaunch !== null;
  const ideContextEnabled =
    !ideContextToggleDisabled &&
    selectedThreadStorageKey !== null &&
    ideContextByThread[selectedThreadStorageKey] === true;

  const selectedThreadIdeContextEnv = useMemo(() => {
    if (!visibleSelectedThread || newThreadLaunch || !selectedThreadStorageKey) {
      return undefined;
    }
    const enabled = ideContextByThread[selectedThreadStorageKey] === true;
    const gitBranch =
      gitBranchInfo?.status === "ok" ? (gitBranchInfo.branch ?? "") : "";
    return buildIdeContextEnv({
      enabled,
      threadKey: selectedThreadStorageKey,
      providerId: visibleSelectedThread.providerId,
      projectPath: visibleSelectedThread.projectPath,
      gitBranch,
    });
  }, [
    gitBranchInfo,
    ideContextByThread,
    newThreadLaunch,
    visibleSelectedThread,
    selectedThreadStorageKey,
  ]);

  const loadOpenTargets = useCallback(async () => {
    setLoadingOpenTargets(true);
    setOpenTargetsLoadError(null);

    try {
      const targets = await invoke<OpenTargetStatus[]>("list_open_targets");
      setOpenTargets(targets);
    } catch (loadError) {
      const message =
        loadError instanceof Error ? loadError.message : String(loadError);
      setOpenTargetsLoadError(message);
      setOpenTargets([]);
    } finally {
      setLoadingOpenTargets(false);
    }
  }, []);

  const loadSophonWorkspacePath = useCallback(async () => {
    setSophonWorkspacePathError(null);
    try {
      const path = await invoke<string>("get_sophon_workspace_path");
      setSophonWorkspacePath(path);
      return path;
    } catch (loadError) {
      const message =
        loadError instanceof Error ? loadError.message : String(loadError);
      setSophonWorkspacePathError(message);
      return null;
    }
  }, []);

  useEffect(() => {
    void loadOpenTargets();
  }, [loadOpenTargets]);

  useEffect(() => {
    void loadSophonWorkspacePath();
  }, [loadSophonWorkspacePath]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    const handleFocus = () => {
      void loadOpenTargets();
    };
    window.addEventListener("focus", handleFocus);
    return () => {
      window.removeEventListener("focus", handleFocus);
    };
  }, [loadOpenTargets]);

  const effectiveDefaultOpenTargetId = useMemo(
    () => resolveDefaultOpenTarget(defaultOpenTargetId, openTargets),
    [defaultOpenTargetId, openTargets],
  );
  const activeProjectUsage = useMemo(() => {
    if (!normalizedActiveProjectPath) {
      return undefined;
    }
    return projectOpenUsage[normalizedActiveProjectPath];
  }, [normalizedActiveProjectPath, projectOpenUsage]);
  const sortedOpenTargets = useMemo(
    () => sortTargetsByProjectUsage(openTargets, activeProjectUsage),
    [activeProjectUsage, openTargets],
  );
  const quickOpenTargetId = useMemo(
    () =>
      resolveQuickOpenTargetId(
        openTargets,
        activeProjectUsage,
        effectiveDefaultOpenTargetId,
      ),
    [activeProjectUsage, effectiveDefaultOpenTargetId, openTargets],
  );

  useEffect(() => {
    if (effectiveDefaultOpenTargetId === defaultOpenTargetId) {
      return;
    }
    setDefaultOpenTargetId(effectiveDefaultOpenTargetId);
  }, [defaultOpenTargetId, effectiveDefaultOpenTargetId]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(
      DEFAULT_OPEN_TARGET_STORAGE_KEY,
      defaultOpenTargetId,
    );
  }, [defaultOpenTargetId]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(
      IDE_CONTEXT_BY_THREAD_STORAGE_KEY,
      serializeIdeContextByThread(ideContextByThread),
    );
  }, [ideContextByThread]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(
      PROJECT_OPEN_USAGE_STORAGE_KEY,
      serializeProjectOpenUsageMap(projectOpenUsage),
    );
  }, [projectOpenUsage]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(APP_WORKSPACE_MODE_KEY, workspaceMode);
  }, [workspaceMode]);

  useEffect(() => {
    if (workspaceMode !== "automatic" || newThreadLaunch !== null) {
      return;
    }
    const nextThreadKey = resolveSelectedThreadKey(automaticModeThreads, selectedThreadKey);
    if (nextThreadKey && nextThreadKey !== selectedThreadKey) {
      handleSelectThread(nextThreadKey);
    }
  }, [
    automaticModeThreads,
    handleSelectThread,
    newThreadLaunch,
    selectedThreadKey,
    workspaceMode,
  ]);

  const loadGitBranch = useCallback(async () => {
    if (!normalizedActiveProjectPath) {
      setGitBranchInfo(null);
      setGitBranchLoading(false);
      return;
    }

    setGitBranchLoading(true);
    try {
      const payload = await invoke<ProjectGitBranchInfo>("get_project_git_branch", {
        request: {
          projectPath: normalizedActiveProjectPath,
        },
      });
      setGitBranchInfo({
        status: payload.status,
        branch: payload.branch ?? null,
        message: payload.message ?? null,
      });
    } catch (branchError) {
      const message =
        branchError instanceof Error ? branchError.message : String(branchError);
      setGitBranchInfo({
        status: "error",
        branch: null,
        message,
      });
    } finally {
      setGitBranchLoading(false);
    }
  }, [normalizedActiveProjectPath]);

  useEffect(() => {
    if (!normalizedActiveProjectPath) {
      setGitBranchInfo(null);
      setGitBranchLoading(false);
      return;
    }

    void loadGitBranch();
    if (typeof window === "undefined") {
      return;
    }

    const intervalId = window.setInterval(() => {
      void loadGitBranch();
    }, GIT_BRANCH_POLL_INTERVAL_MS);
    const handleFocus = () => {
      void loadGitBranch();
    };

    window.addEventListener("focus", handleFocus);
    return () => {
      window.clearInterval(intervalId);
      window.removeEventListener("focus", handleFocus);
    };
  }, [loadGitBranch, normalizedActiveProjectPath]);

  const handleDefaultOpenTargetChange = useCallback((targetId: OpenTargetId) => {
    setDefaultOpenTargetId(targetId);
  }, []);

  const handleOpenWithTarget = useCallback(
    async (targetId: OpenTargetId) => {
      if (!normalizedActiveProjectPath) {
        setOpenTargetActionError(
          "Select a thread with a valid project path before opening in a target.",
        );
        return;
      }

      setOpenTargetActionError(null);
      setOpeningTargetId(targetId);
      try {
        await invoke<OpenProjectWithTargetResponse>("open_project_with_target", {
          request: {
            projectPath: normalizedActiveProjectPath,
            targetId,
          },
        });
        setProjectOpenUsage((current) =>
          updateProjectOpenUsage(current, normalizedActiveProjectPath, targetId),
        );
        void loadGitBranch();
      } catch (openError) {
        const message =
          openError instanceof Error ? openError.message : String(openError);
        setOpenTargetActionError(message);
      } finally {
        setOpeningTargetId(null);
      }
    },
    [loadGitBranch, normalizedActiveProjectPath],
  );

  useEffect(() => {
    setOpenTargetActionError(null);
  }, [normalizedActiveProjectPath]);

  const handleIdeContextEnabledChange = useCallback(
    (enabled: boolean) => {
      if (!selectedThreadStorageKey || newThreadLaunch) {
        return;
      }
      setIdeContextByThread((current) =>
        setThreadIdeContextEnabled(current, selectedThreadStorageKey, enabled),
      );
    },
    [newThreadLaunch, selectedThreadStorageKey],
  );

  const developerActionError = openTargetActionError ?? openTargetsLoadError;

  const { dragRegionRef, windowDragStripHeight } = useWindowDrag();

  const handleSelectThreadView = useCallback(
    (nextThreadKey: string) => {
      handleSelectThread(nextThreadKey);
    },
    [handleSelectThread],
  );

  const handleCreateThreadView = useCallback(
    async (
      projectPath: string,
      providerId: ThreadProviderId,
      profileName: string,
      launchEnv?: Record<string, string>,
    ) => {
      await handleCreateThreadInFolder(projectPath, providerId, profileName, launchEnv);
    },
    [handleCreateThreadInFolder],
  );

  const handleWorkspaceModeChange = useCallback((nextMode: AppWorkspaceMode) => {
    setWorkspaceMode(nextMode);
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(APP_THEME_KEY, appTheme);
  }, [appTheme]);

  const resolveProfileNameForProvider = (providerId: string): string => {
    if (!isSupportedProvider(providerId)) {
      return activeProfileName;
    }
    return providerProfiles[providerId] ?? "default";
  };

  const resolveLaunchEnv = (providerId: string): Record<string, string> | undefined => {
    if (!isSupportedProvider(providerId)) {
      return undefined;
    }
    return resolveLaunchEnvForProvider(agentRuntimeSettings, providerId);
  };

  const handleCreateAutomaticThread = useCallback(async () => {
    const workspacePath = automaticModeWorkspacePath;
    if (!workspacePath) {
      setError(sophonWorkspacePathError ?? "Sophon workspace path is not available yet.");
      return;
    }

    await handleCreateThreadView(
      workspacePath,
      "sophon",
      resolveProfileNameForProvider("sophon"),
      resolveLaunchEnv("sophon"),
    );
  }, [
    automaticModeWorkspacePath,
    handleCreateThreadView,
    resolveLaunchEnv,
    resolveProfileNameForProvider,
    setError,
    sophonWorkspacePathError,
  ]);

  const handleAgentRuntimeSettingsChange = async (
    nextSettings: AgentRuntimeSettings,
  ): Promise<string | null> => {
    const normalized = normalizeAgentRuntimeSettings(nextSettings);
    const errorMessage = validateAgentRuntimeSettings(normalized);
    if (errorMessage) {
      return errorMessage;
    }

    if (hasSophonSupplierChanged(agentRuntimeSettings, normalized)) {
      try {
        await invoke<SyncSophonAccountResult>("sync_sophon_account_settings", {
          request: buildSophonSyncRequest(normalized),
        });
      } catch (error) {
        return error instanceof Error ? error.message : String(error);
      }
    }

    if (typeof window !== "undefined") {
      try {
        window.localStorage.setItem(
          AGENT_RUNTIME_SETTINGS_KEY,
          JSON.stringify(normalized),
        );
      } catch (error) {
        if (error instanceof Error) {
          return error.message;
        }
        return String(error);
      }
    }

    setAgentRuntimeSettings(normalized);
    return null;
  };

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = (event: MediaQueryListEvent) => {
      setSystemTheme(event.matches ? "dark" : "light");
    };

    setSystemTheme(mediaQuery.matches ? "dark" : "light");
    mediaQuery.addEventListener("change", handleChange);
    return () => {
      mediaQuery.removeEventListener("change", handleChange);
    };
  }, []);

  useEffect(() => {
    if (typeof document === "undefined") {
      return;
    }
    const root = document.documentElement;
    root.classList.toggle("dark", resolvedTheme === "dark");
    root.classList.toggle("brutalism", appSkin === "brutalism");
    root.style.colorScheme = resolvedTheme;
  }, [resolvedTheme, appSkin]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(APP_SKIN_KEY, appSkin);
  }, [appSkin]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(MAIN_VIEW_KEY, mainView);
  }, [mainView]);

  return (
    <main className="relative h-full min-h-0 select-none overflow-hidden bg-background">
      <div
        ref={dragRegionRef}
        data-window-drag-region="true"
        className="absolute left-0 right-0 top-0 z-[9999] select-none"
        style={{ height: windowDragStripHeight }}
      />
      <section
        ref={layoutRef}
        className="grid h-full min-h-0 flex-1 overflow-hidden"
        style={layoutGridStyle}
      >
        <Sidebar
          sidebarCollapsed={sidebarCollapsed}
          folderGroups={folderGroups}
          automaticThreads={automaticModeThreads}
          workspaceMode={workspaceMode}
          automaticWorkspacePath={automaticModeWorkspacePath}
          automaticWorkspacePathError={sophonWorkspacePathError}
          selectedFolderKey={selectedFolderKey}
          selectedThreadKey={workspaceMode === "automatic" ? visibleSelectedThreadKey : selectedThreadKey}
          loadingThreads={loadingThreads}
          creatingThreadFolderKey={creatingThreadFolderKey}
          error={error}
          newThreadBindingStatus={newThreadBindingStatus}
          hasPendingNewThreadLaunch={newThreadLaunch !== null}
          appTheme={appTheme}
          appSkin={appSkin}
          activeProviderId={activeProviderId}
          activeProfileName={activeProfileName}
          providerProfiles={providerProfiles}
          agentRuntimeSettings={agentRuntimeSettings}
          onLoadThreads={loadThreads}
          onSelectThread={handleSelectThreadView}
          onCreateThread={(projectPath, providerId) =>
            handleCreateThreadView(
              projectPath,
              providerId,
              resolveProfileNameForProvider(providerId),
              resolveLaunchEnv(providerId),
            )
          }
          onCreateAutomaticThread={handleCreateAutomaticThread}
          onWorkspaceModeChange={handleWorkspaceModeChange}
          onAgentRuntimeSettingsChange={handleAgentRuntimeSettingsChange}
          onAppThemeChange={setAppTheme}
          onAppSkinChange={setAppSkin}
          onClearError={() => setError(null)}
        />

        {!sidebarCollapsed ? (
          <div
            role="separator"
            aria-orientation="vertical"
            className={cn(
              "group flex h-full cursor-col-resize items-center justify-center",
              isResizingSidebar ? "bg-primary/10" : "hover:bg-primary/5",
            )}
            onMouseDown={handleSidebarResizeStart}
          >
            <span
              className={cn(
                "h-14 w-[2px] rounded-full bg-border transition-colors",
                isResizingSidebar
                  ? "bg-primary/55"
                  : "group-hover:bg-primary/45",
              )}
            />
          </div>
        ) : null}

        <Card
          className={cn(
            "flex min-h-0 min-w-0 flex-col rounded-none rounded-tl-xl border-0 bg-card shadow-none",
            sidebarCollapsed ? "col-start-1" : "col-start-3",
          )}
        >
          <ThreadHeader
            sidebarCollapsed={sidebarCollapsed}
            selectedThread={visibleSelectedThread}
            newThreadLaunch={newThreadLaunch}
            newThreadBindingStatus={newThreadBindingStatus}
            openTargets={sortedOpenTargets}
            loadingOpenTargets={loadingOpenTargets}
            defaultOpenTargetId={effectiveDefaultOpenTargetId}
            quickOpenTargetId={quickOpenTargetId}
            openingTargetId={openingTargetId}
            openTargetError={developerActionError}
            onOpenWithTarget={handleOpenWithTarget}
            onDefaultOpenTargetChange={handleDefaultOpenTargetChange}
            ideContextEnabled={ideContextEnabled}
            ideContextToggleDisabled={ideContextToggleDisabled}
            onIdeContextEnabledChange={handleIdeContextEnabledChange}
            gitBranchInfo={gitBranchInfo}
            gitBranchLoading={gitBranchLoading}
            onToggleSidebar={toggleSidebar}
          />
          <Separator />

          <div className="flex items-center gap-2 border-b bg-card px-4 py-2">
            <Button
              type="button"
              size="sm"
              variant={mainView === "terminal" ? "default" : "ghost"}
              onClick={() => setMainView("terminal")}
            >
              Terminal
            </Button>
            <Button
              type="button"
              size="sm"
              variant={mainView === "dock_agent" ? "default" : "ghost"}
              onClick={() => setMainView("dock_agent")}
            >
              Dock Agent
            </Button>
          </div>

          <CardContent className={cn("min-h-0 flex-1", "p-0")}>
            <div className="h-full w-full">
              {mainView === "terminal" ? (
                <EmbeddedTerminal
                  thread={
                    newThreadLaunch
                      ? {
                          id: `__new__:${newThreadLaunch.launchId}`,
                          providerId: newThreadLaunch.providerId,
                          profileName: newThreadLaunch.profileName,
                          launchEnv: newThreadLaunch.launchEnv,
                          projectPath: newThreadLaunch.projectPath,
                        }
                      : visibleSelectedThread
                        ? {
                            id: visibleSelectedThread.id,
                            providerId: visibleSelectedThread.providerId,
                            profileName: resolveProfileNameForProvider(visibleSelectedThread.providerId),
                            launchEnv: resolveLaunchEnv(visibleSelectedThread.providerId),
                            ideContextEnv: selectedThreadIdeContextEnv,
                            projectPath: visibleSelectedThread.projectPath,
                          }
                        : null
                  }
                  launchRequest={newThreadLaunch}
                  terminalTheme={resolvedTheme}
                  appSkin={appSkin}
                  onLaunchRequestSettled={handleNewThreadLaunchSettled}
                  onActiveSessionExit={handleEmbeddedTerminalSessionExit}
                  onError={setError}
                />
              ) : (
                <DockAgentPanel
                  selectedThread={visibleSelectedThread}
                  darkMode={resolvedTheme === "dark"}
                />
              )}
            </div>
          </CardContent>
        </Card>
      </section>
    </main>
  );
}

export default App;
