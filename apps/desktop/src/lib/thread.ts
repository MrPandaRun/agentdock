import type { AgentThreadSummary } from "@/types";

export function formatLastActive(raw: string): string {
  const value = toTimestampMs(raw);
  if (!Number.isFinite(value)) {
    return raw;
  }

  return new Intl.DateTimeFormat("zh-CN", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

export function toTimestampMs(raw: string): number {
  const numeric = Number(raw);

  if (Number.isFinite(numeric)) {
    return numeric < 1_000_000_000_000 ? numeric * 1000 : numeric;
  }
  return Date.parse(raw);
}

export function sortableTimestamp(raw: string): number {
  const timestamp = toTimestampMs(raw);
  return Number.isFinite(timestamp) ? timestamp : 0;
}

export function normalizeProjectPath(path: string): string {
  const trimmed = path.trim();
  if (!trimmed) {
    return ".";
  }
  return trimmed;
}

export function folderNameFromProjectPath(path: string): string {
  const normalized = normalizeProjectPath(path).replace(/\\/g, "/");
  if (normalized === ".") {
    return "Unknown folder";
  }
  const segments = normalized.split("/").filter(Boolean);
  return segments[segments.length - 1] ?? normalized;
}

export function threadPreview(
  thread: Pick<AgentThreadSummary, "title" | "lastMessagePreview">,
): string {
  const title = thread.title.trim();
  if (title) {
    return title;
  }

  const preview = thread.lastMessagePreview?.trim();
  if (preview) {
    return preview;
  }
  return "Untitled thread";
}

export function threadKey(
  thread: Pick<AgentThreadSummary, "providerId" | "id">,
): string {
  return `${thread.providerId}:${thread.id}`;
}

export function resolveSelectedThreadKey(
  threads: AgentThreadSummary[],
  current: string | null,
): string | null {
  const visibleThreads = threads.filter(
    (thread) => normalizeProjectPath(thread.projectPath) !== ".",
  );
  if (current && visibleThreads.some((thread) => threadKey(thread) === current)) {
    return current;
  }
  if (visibleThreads.length > 0) {
    return threadKey(visibleThreads[0]);
  }
  return threads[0] ? threadKey(threads[0]) : null;
}

export interface PickCreatedThreadLaunch {
  providerId: string;
  projectPath: string;
  knownThreadKeys: string[];
}

export function pickCreatedThread(
  threads: AgentThreadSummary[],
  launch: PickCreatedThreadLaunch,
): AgentThreadSummary | null {
  const normalizedProjectPath = normalizeProjectPath(launch.projectPath);
  const matches = threads.filter(
    (thread) =>
      thread.providerId === launch.providerId &&
      normalizeProjectPath(thread.projectPath) === normalizedProjectPath,
  );
  if (matches.length === 0) {
    return null;
  }

  const knownThreadKeys = new Set(launch.knownThreadKeys);
  const freshMatches = matches.filter(
    (thread) => !knownThreadKeys.has(threadKey(thread)),
  );
  if (freshMatches.length === 0) {
    return null;
  }

  return (
    [...freshMatches].sort(
      (a, b) => sortableTimestamp(b.lastActiveAt) - sortableTimestamp(a.lastActiveAt),
    )[0] ?? null
  );
}
