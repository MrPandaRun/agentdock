import type { OpenTargetId, OpenTargetStatus } from "@/types";

export const DEFAULT_OPEN_TARGET_STORAGE_KEY = "agentdock.desktop.default_open_target";
export const IDE_CONTEXT_BY_THREAD_STORAGE_KEY = "agentdock.desktop.ide_context_by_thread";
export const PROJECT_OPEN_USAGE_STORAGE_KEY = "agentdock.desktop.project_open_usage";

const DEFAULT_OPEN_TARGET_ID: OpenTargetId = "vscode";

const OPEN_TARGET_IDS: OpenTargetId[] = [
  "vscode",
  "cursor",
  "windsurf",
  "antigravity",
  "zed",
  "intellij",
  "webstorm",
  "pycharm",
  "sublime_text",
  "terminal",
  "iterm",
  "warp",
];

const OPEN_TARGET_ID_SET = new Set<OpenTargetId>(OPEN_TARGET_IDS);
const OPEN_TARGET_ORDER_INDEX = new Map<OpenTargetId, number>(
  OPEN_TARGET_IDS.map((targetId, index) => [targetId, index]),
);

export interface BuildIdeContextEnvOptions {
  enabled: boolean;
  threadKey: string;
  providerId: string;
  projectPath: string;
  gitBranch?: string | null;
}

export type ProjectOpenUsageMap = Record<string, Partial<Record<OpenTargetId, number>>>;

export function isOpenTargetId(value: string): value is OpenTargetId {
  return OPEN_TARGET_ID_SET.has(value as OpenTargetId);
}

export function normalizeDefaultOpenTarget(value: unknown): OpenTargetId {
  if (typeof value === "string" && isOpenTargetId(value)) {
    return value;
  }
  return DEFAULT_OPEN_TARGET_ID;
}

export function resolveDefaultOpenTarget(
  preferred: OpenTargetId,
  targets: OpenTargetStatus[],
): OpenTargetId {
  const preferredAvailable = targets.some(
    (target) => target.id === preferred && target.available,
  );
  if (preferredAvailable) {
    return preferred;
  }
  const firstAvailable = targets.find((target) => target.available);
  if (firstAvailable) {
    return firstAvailable.id;
  }
  return preferred;
}

export function parseIdeContextByThread(value: unknown): Record<string, boolean> {
  if (typeof value !== "string" || value.trim().length === 0) {
    return {};
  }
  try {
    const parsed = JSON.parse(value) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return {};
    }
    const result: Record<string, boolean> = {};
    for (const [key, raw] of Object.entries(parsed as Record<string, unknown>)) {
      if (typeof key !== "string") {
        continue;
      }
      if (raw === true) {
        result[key] = true;
      }
    }
    return result;
  } catch {
    return {};
  }
}

export function serializeIdeContextByThread(map: Record<string, boolean>): string {
  const normalized: Record<string, boolean> = {};
  for (const [key, enabled] of Object.entries(map)) {
    if (!enabled) {
      continue;
    }
    normalized[key] = true;
  }
  return JSON.stringify(normalized);
}

export function setThreadIdeContextEnabled(
  current: Record<string, boolean>,
  threadKey: string,
  enabled: boolean,
): Record<string, boolean> {
  if (!threadKey.trim()) {
    return current;
  }
  if (enabled) {
    return {
      ...current,
      [threadKey]: true,
    };
  }

  const next = { ...current };
  delete next[threadKey];
  return next;
}

export function buildIdeContextEnv(
  options: BuildIdeContextEnvOptions,
): Record<string, string> | undefined {
  if (!options.enabled) {
    return undefined;
  }

  return {
    AGENTDOCK_IDE_CONTEXT_ENABLED: "1",
    AGENTDOCK_IDE_CONTEXT_THREAD_KEY: options.threadKey,
    AGENTDOCK_IDE_CONTEXT_PROVIDER_ID: options.providerId,
    AGENTDOCK_IDE_CONTEXT_PROJECT_PATH: options.projectPath,
    AGENTDOCK_IDE_CONTEXT_GIT_BRANCH: options.gitBranch?.trim() ?? "",
  };
}

export function parseProjectOpenUsageMap(value: unknown): ProjectOpenUsageMap {
  if (typeof value !== "string" || value.trim().length === 0) {
    return {};
  }

  try {
    const parsed = JSON.parse(value) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return {};
    }

    const result: ProjectOpenUsageMap = {};
    for (const [projectPath, rawUsage] of Object.entries(parsed as Record<string, unknown>)) {
      if (typeof projectPath !== "string" || !rawUsage || typeof rawUsage !== "object") {
        continue;
      }
      const usageRecord: Partial<Record<OpenTargetId, number>> = {};
      for (const [targetId, rawTime] of Object.entries(rawUsage as Record<string, unknown>)) {
        if (!isOpenTargetId(targetId)) {
          continue;
        }
        if (typeof rawTime !== "number" || !Number.isFinite(rawTime) || rawTime <= 0) {
          continue;
        }
        usageRecord[targetId] = rawTime;
      }
      if (Object.keys(usageRecord).length > 0) {
        result[projectPath] = usageRecord;
      }
    }
    return result;
  } catch {
    return {};
  }
}

export function serializeProjectOpenUsageMap(map: ProjectOpenUsageMap): string {
  return JSON.stringify(map);
}

export function updateProjectOpenUsage(
  current: ProjectOpenUsageMap,
  projectPath: string,
  targetId: OpenTargetId,
  openedAt = Date.now(),
): ProjectOpenUsageMap {
  if (!projectPath.trim()) {
    return current;
  }

  return {
    ...current,
    [projectPath]: {
      ...(current[projectPath] ?? {}),
      [targetId]: openedAt,
    },
  };
}

export function sortTargetsByProjectUsage(
  targets: OpenTargetStatus[],
  usage: Partial<Record<OpenTargetId, number>> | undefined,
): OpenTargetStatus[] {
  return [...targets].sort((a, b) => {
    const aInstallRank = a.installed ? 0 : 1;
    const bInstallRank = b.installed ? 0 : 1;
    if (aInstallRank !== bInstallRank) {
      return aInstallRank - bInstallRank;
    }

    const aUsedAt = usage?.[a.id] ?? 0;
    const bUsedAt = usage?.[b.id] ?? 0;
    if (aUsedAt !== bUsedAt) {
      return bUsedAt - aUsedAt;
    }

    if (a.available !== b.available) {
      return a.available ? -1 : 1;
    }

    const aOrder = OPEN_TARGET_ORDER_INDEX.get(a.id) ?? Number.MAX_SAFE_INTEGER;
    const bOrder = OPEN_TARGET_ORDER_INDEX.get(b.id) ?? Number.MAX_SAFE_INTEGER;
    return aOrder - bOrder;
  });
}

export function resolveQuickOpenTargetId(
  targets: OpenTargetStatus[],
  usage: Partial<Record<OpenTargetId, number>> | undefined,
  fallback: OpenTargetId,
): OpenTargetId {
  const availableIds = new Set(
    targets.filter((target) => target.available).map((target) => target.id),
  );

  if (usage) {
    let bestId: OpenTargetId | null = null;
    let bestTime = 0;
    for (const [targetId, rawTime] of Object.entries(usage)) {
      if (!isOpenTargetId(targetId) || !availableIds.has(targetId)) {
        continue;
      }
      const openedAt = typeof rawTime === "number" ? rawTime : 0;
      if (openedAt > bestTime) {
        bestTime = openedAt;
        bestId = targetId;
      }
    }
    if (bestId) {
      return bestId;
    }
  }

  return resolveDefaultOpenTarget(fallback, targets);
}
