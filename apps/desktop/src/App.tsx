import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  ArrowUp,
  Bot,
  Code2,
  Monitor,
  PanelLeftClose,
  PanelLeftOpen,
  Paperclip,
  Pencil,
  RefreshCw,
  SquareTerminal,
  Slash,
} from "lucide-react";
import {
  type CSSProperties,
  FormEvent,
  KeyboardEvent,
  MouseEvent as ReactMouseEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  EmbeddedTerminal,
  type EmbeddedTerminalNewThreadLaunch,
} from "@/components/terminal/EmbeddedTerminal";
import {
  ThreadFolderGroup,
  type ThreadFolderGroupItem,
} from "@/components/threads/ThreadFolderGroup";
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";

interface AgentThreadSummary {
  id: string;
  providerId: "claude_code" | string;
  projectPath: string;
  title: string;
  tags: string[];
  lastActiveAt: string;
  lastMessagePreview?: string | null;
}

interface AgentThreadMessage {
  role: string;
  content: string;
  timestampMs?: number;
  kind: "text" | "tool" | string;
  collapsed: boolean;
}

interface SendClaudeMessageResponse {
  threadId: string;
  responseText: string;
  rawOutput: string;
}

interface ToolMessageParts {
  headline: string;
  detail?: string;
  ioLabel?: "IN" | "OUT";
  ioBody?: string;
}

const SIDEBAR_WIDTH_KEY = "agentdock.desktop.sidebar_width";
const SIDEBAR_COLLAPSED_KEY = "agentdock.desktop.sidebar_collapsed";
const RIGHT_PANE_MODE_KEY = "agentdock.desktop.right_pane_mode";
const MIN_SIDEBAR_WIDTH = 240;
const MAX_SIDEBAR_WIDTH = 520;
const WINDOW_DRAG_STRIP_HEIGHT = 32;

type RightPaneMode = "terminal" | "ui";
type ThreadProviderId = "claude_code" | "codex" | "opencode";

function readStoredSidebarWidth(): number {
  if (typeof window === "undefined") {
    return 300;
  }
  const raw = window.localStorage.getItem(SIDEBAR_WIDTH_KEY);
  const parsed = Number(raw);
  if (!Number.isFinite(parsed)) {
    return 300;
  }
  return Math.min(Math.max(parsed, MIN_SIDEBAR_WIDTH), MAX_SIDEBAR_WIDTH);
}

function readStoredSidebarCollapsed(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  return window.localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === "1";
}

function readStoredRightPaneMode(): RightPaneMode {
  if (typeof window === "undefined") {
    return "terminal";
  }
  const raw = window.localStorage.getItem(RIGHT_PANE_MODE_KEY);
  return raw === "ui" ? "ui" : "terminal";
}

function formatLastActive(raw: string): string {
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

function toTimestampMs(raw: string): number {
  const numeric = Number(raw);

  if (Number.isFinite(numeric)) {
    return numeric < 1_000_000_000_000 ? numeric * 1000 : numeric;
  }
  return Date.parse(raw);
}

function sortableTimestamp(raw: string): number {
  const timestamp = toTimestampMs(raw);
  return Number.isFinite(timestamp) ? timestamp : 0;
}

function normalizeProjectPath(path: string): string {
  const trimmed = path.trim();
  if (!trimmed) {
    return ".";
  }
  return trimmed;
}

function folderNameFromProjectPath(path: string): string {
  const normalized = normalizeProjectPath(path).replace(/\\/g, "/");
  if (normalized === ".") {
    return "Unknown folder";
  }
  const segments = normalized.split("/").filter(Boolean);
  return segments[segments.length - 1] ?? normalized;
}

function threadPreview(thread: Pick<AgentThreadSummary, "title" | "lastMessagePreview">): string {
  const preview = thread.lastMessagePreview?.trim();
  if (preview) {
    return preview;
  }
  return thread.title;
}

function resolveSelectedThreadId(
  threads: AgentThreadSummary[],
  current: string | null,
): string | null {
  const visibleThreads = threads.filter(
    (thread) => normalizeProjectPath(thread.projectPath) !== ".",
  );
  if (current && visibleThreads.some((thread) => thread.id === current)) {
    return current;
  }
  return visibleThreads[0]?.id ?? threads[0]?.id ?? null;
}

function pickCreatedThread(
  threads: AgentThreadSummary[],
  launch: EmbeddedTerminalNewThreadLaunch,
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

  const knownIds = new Set(launch.knownThreadIds);
  const freshMatches = matches.filter((thread) => !knownIds.has(thread.id));
  const candidates = freshMatches.length > 0 ? freshMatches : matches;
  return (
    [...candidates].sort(
      (a, b) => sortableTimestamp(b.lastActiveAt) - sortableTimestamp(a.lastActiveAt),
    )[0] ?? null
  );
}

function isCodexProvider(providerId?: string): boolean {
  return providerId === "codex";
}

function isOpenCodeProvider(providerId?: string): boolean {
  return providerId === "opencode";
}

function providerDisplayName(providerId?: string): string {
  if (providerId === "codex") {
    return "Codex";
  }
  if (providerId === "opencode") {
    return "OpenCode";
  }
  if (providerId === "claude_code") {
    return "Claude Code";
  }
  return providerId ?? "Provider";
}

function providerAccentClass(providerId?: string): string {
  if (isCodexProvider(providerId)) {
    return "text-[hsl(var(--brand-codex))]";
  }
  if (isOpenCodeProvider(providerId)) {
    return "text-[hsl(var(--brand-opencode))]";
  }
  return "text-[hsl(var(--brand-claude))]";
}

function splitToolMessage(content: string): ToolMessageParts {
  const lines = content
    .split("\n")
    .map((line) => line.replace(/\r/g, "").trimEnd())
    .filter((line) => line.trim().length > 0);

  if (lines.length === 0) {
    return { headline: "" };
  }

  const firstLine = lines[0].trim();
  const restLines = lines.slice(1);
  const ioFromFirst = parseIoLine(firstLine);
  if (ioFromFirst && restLines.length === 0) {
    return {
      headline: ioFromFirst.label,
      ioLabel: ioFromFirst.label,
      ioBody: ioFromFirst.body,
    };
  }

  const firstDetailLine = restLines[0]?.trim();
  const ioFromDetail = firstDetailLine ? parseIoLine(firstDetailLine) : null;
  if (ioFromDetail) {
    const blockLines = [ioFromDetail.body, ...restLines.slice(1)].filter(
      (line) => line.trim().length > 0,
    );
    return {
      headline: firstLine,
      ioLabel: ioFromDetail.label,
      ioBody: blockLines.join("\n"),
    };
  }

  return {
    headline: firstLine,
    detail: restLines.join("\n"),
  };
}

function parseIoLine(line: string): { label: "IN" | "OUT"; body: string } | null {
  if (line.startsWith("IN ")) {
    return { label: "IN", body: line.slice(3).trim() };
  }
  if (line === "IN") {
    return { label: "IN", body: "" };
  }
  if (line.startsWith("OUT ")) {
    return { label: "OUT", body: line.slice(4).trim() };
  }
  if (line === "OUT") {
    return { label: "OUT", body: "" };
  }
  return null;
}

function normalizeCodeBody(raw: string): string {
  return raw
    .split("\n")
    .map((line) => line.replace(/\t/g, "  "))
    .join("\n")
    .trim();
}

function parseToolTitle(raw: string): { strong: string; rest?: string } {
  const line = raw.trim();
  if (!line) {
    return { strong: "" };
  }
  const firstSpace = line.indexOf(" ");
  if (firstSpace === -1) {
    return { strong: line };
  }
  return {
    strong: line.slice(0, firstSpace),
    rest: line.slice(firstSpace + 1).trim(),
  };
}

function makeLocalMessage(role: string, content: string): AgentThreadMessage {
  return {
    role,
    content,
    timestampMs: Date.now(),
    kind: "text",
    collapsed: false,
  };
}

function App() {
  const [threads, setThreads] = useState<AgentThreadSummary[]>([]);
  const [selectedThreadId, setSelectedThreadId] = useState<string | null>(null);
  const [messages, setMessages] = useState<AgentThreadMessage[]>([]);
  const [draft, setDraft] = useState("");
  const [loadingThreads, setLoadingThreads] = useState(true);
  const [loadingMessages, setLoadingMessages] = useState(false);
  const [sending, setSending] = useState(false);
  const [showToolEvents, setShowToolEvents] = useState(false);
  const [rightPaneMode, setRightPaneMode] = useState<RightPaneMode>(
    readStoredRightPaneMode,
  );
  const [error, setError] = useState<string | null>(null);
  const [creatingThreadFolderKey, setCreatingThreadFolderKey] = useState<string | null>(
    null,
  );
  const [newThreadLaunch, setNewThreadLaunch] = useState<EmbeddedTerminalNewThreadLaunch | null>(
    null,
  );
  const [sidebarWidth, setSidebarWidth] = useState<number>(readStoredSidebarWidth);
  const [sidebarCollapsed, setSidebarCollapsed] = useState<boolean>(
    readStoredSidebarCollapsed,
  );
  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const layoutRef = useRef<HTMLElement | null>(null);
  const resizeStateRef = useRef<{ startX: number; startWidth: number } | null>(null);
  const dragRegionRef = useRef<HTMLDivElement>(null);
  const pendingNewThreadLaunchIdRef = useRef<number | null>(null);
  const appWindow = useMemo(() => getCurrentWindow(), []);

  const selectedThread = useMemo(
    () => threads.find((thread) => thread.id === selectedThreadId) ?? null,
    [threads, selectedThreadId],
  );

  const folderGroups = useMemo<ThreadFolderGroupItem<AgentThreadSummary>[]>(() => {
    const grouped = new Map<string, AgentThreadSummary[]>();

    for (const thread of threads) {
      const key = normalizeProjectPath(thread.projectPath);
      const items = grouped.get(key);
      if (items) {
        items.push(thread);
      } else {
        grouped.set(key, [thread]);
      }
    }

    return [...grouped.entries()]
      .map(([key, items]) => {
        const sorted = [...items].sort((a, b) => {
          return sortableTimestamp(b.lastActiveAt) - sortableTimestamp(a.lastActiveAt);
        });
        return {
          key,
          folderName: folderNameFromProjectPath(key),
          threads: sorted,
        };
      })
      .filter((group) => group.key !== ".")
      .sort((a, b) => {
        const aLatest = a.threads[0]?.lastActiveAt ?? "";
        const bLatest = b.threads[0]?.lastActiveAt ?? "";
        return sortableTimestamp(bLatest) - sortableTimestamp(aLatest);
      });
  }, [threads]);

  const selectedFolderKey = useMemo(() => {
    if (!selectedThread) {
      return null;
    }
    return normalizeProjectPath(selectedThread.projectPath);
  }, [selectedThread]);
  const canUseUiComposer = selectedThread?.providerId === "claude_code";
  const headerProviderId =
    rightPaneMode === "terminal" && newThreadLaunch
      ? newThreadLaunch.providerId
      : selectedThread?.providerId;
  const headerTitle =
    rightPaneMode === "terminal" && newThreadLaunch
      ? `New ${providerDisplayName(newThreadLaunch.providerId)} thread`
      : (selectedThread?.title ?? "Select a thread");
  const headerProjectPath =
    rightPaneMode === "terminal" && newThreadLaunch
      ? newThreadLaunch.projectPath
      : (selectedThread?.projectPath ?? "-");
  const headerProviderName = providerDisplayName(headerProviderId);
  const headerProviderAccent = providerAccentClass(headerProviderId);

  const displayedMessages = useMemo(() => {
    if (showToolEvents) {
      return messages;
    }
    return messages.filter((message) => message.kind !== "tool");
  }, [messages, showToolEvents]);

  const toolCount = useMemo(
    () => messages.filter((message) => message.kind === "tool").length,
    [messages],
  );

  const layoutGridStyle = useMemo<CSSProperties>(() => {
    if (sidebarCollapsed) {
      return { gridTemplateColumns: "minmax(0, 1fr)" };
    }
    return {
      gridTemplateColumns: `${sidebarWidth}px 10px minmax(0, 1fr)`,
    };
  }, [sidebarCollapsed, sidebarWidth]);

  const clampSidebarWidth = useCallback((value: number): number => {
    const layoutWidth = layoutRef.current?.clientWidth;
    const maxByContainer = layoutWidth
      ? Math.max(MIN_SIDEBAR_WIDTH, layoutWidth - 420)
      : MAX_SIDEBAR_WIDTH;
    const maxAllowed = Math.min(MAX_SIDEBAR_WIDTH, maxByContainer);
    return Math.min(Math.max(value, MIN_SIDEBAR_WIDTH), maxAllowed);
  }, []);

  const loadThreads = useCallback(async () => {
    setLoadingThreads(true);
    setError(null);
    try {
      const data = await invoke<AgentThreadSummary[]>("list_threads");
      setThreads(data);
      setSelectedThreadId((current) => resolveSelectedThreadId(data, current));
    } catch (loadError) {
      const message =
        loadError instanceof Error ? loadError.message : String(loadError);
      setError(message);
    } finally {
      setLoadingThreads(false);
    }
  }, []);

  const loadMessages = useCallback(async (threadId: string, providerId: string) => {
    const data = await invoke<AgentThreadMessage[]>("get_thread_messages", {
      request: {
        threadId,
        providerId,
      },
    });
    return data;
  }, []);

  const handleCreateThreadInFolder = useCallback(
    async (projectPath: string, providerId: ThreadProviderId) => {
      const launchId = Date.now();
      setCreatingThreadFolderKey(projectPath);
      setError(null);
      setRightPaneMode("terminal");
      pendingNewThreadLaunchIdRef.current = launchId;
      setNewThreadLaunch({
        launchId,
        providerId,
        projectPath,
        knownThreadIds: threads.map((thread) => thread.id),
      });
    },
    [threads],
  );

  const handleSelectThread = useCallback((threadId: string) => {
    pendingNewThreadLaunchIdRef.current = null;
    setCreatingThreadFolderKey(null);
    setNewThreadLaunch(null);
    setSelectedThreadId(threadId);
  }, []);

  const handleNewThreadLaunchSettled = useCallback(
    (launch: EmbeddedTerminalNewThreadLaunch) => {
      const retryDelaysMs = [350, 700, 1000, 1200];

      const settle = () => {
        const isActiveLaunch = pendingNewThreadLaunchIdRef.current === launch.launchId;
        if (isActiveLaunch) {
          pendingNewThreadLaunchIdRef.current = null;
        }
        setCreatingThreadFolderKey((current) => {
          if (!isActiveLaunch) {
            return current;
          }
          return current === launch.projectPath ? null : current;
        });
        setNewThreadLaunch((current) =>
          current?.launchId === launch.launchId ? null : current,
        );
      };

      void (async () => {
        try {
          for (let attempt = 0; attempt <= retryDelaysMs.length; attempt += 1) {
            if (pendingNewThreadLaunchIdRef.current !== launch.launchId) {
              return;
            }

            const data = await invoke<AgentThreadSummary[]>("list_threads");
            const createdThread = pickCreatedThread(data, launch);
            setThreads(data);

            if (createdThread) {
              setSelectedThreadId(createdThread.id);
              settle();
              return;
            }

            setSelectedThreadId((current) => resolveSelectedThreadId(data, current));

            if (attempt < retryDelaysMs.length) {
              await new Promise((resolve) => {
                window.setTimeout(resolve, retryDelaysMs[attempt]);
              });
            }
          }
        } catch (refreshError) {
          if (pendingNewThreadLaunchIdRef.current === launch.launchId) {
            const message =
              refreshError instanceof Error ? refreshError.message : String(refreshError);
            setError(message);
          }
        } finally {
          settle();
        }
      })();
    },
    [],
  );

  useEffect(() => {
    void loadThreads();
  }, [loadThreads]);

  useEffect(() => {
    if (rightPaneMode === "terminal") {
      return;
    }
    pendingNewThreadLaunchIdRef.current = null;
    setNewThreadLaunch(null);
    setCreatingThreadFolderKey(null);
  }, [rightPaneMode]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth));
  }, [sidebarWidth]);

  useEffect(() => {
    const handleResize = () => {
      setSidebarWidth((current) => clampSidebarWidth(current));
    };
    handleResize();
    window.addEventListener("resize", handleResize);
    return () => {
      window.removeEventListener("resize", handleResize);
    };
  }, [clampSidebarWidth]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(
      SIDEBAR_COLLAPSED_KEY,
      sidebarCollapsed ? "1" : "0",
    );
  }, [sidebarCollapsed]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(RIGHT_PANE_MODE_KEY, rightPaneMode);
  }, [rightPaneMode]);

  useEffect(() => {
    if (rightPaneMode !== "ui") {
      setLoadingMessages(false);
      return;
    }

    if (!selectedThreadId) {
      setMessages([]);
      return;
    }

    let active = true;
    setLoadingMessages(true);
    setError(null);

    const providerId = selectedThread?.providerId;
    if (!providerId) {
      setMessages([]);
      setLoadingMessages(false);
      return;
    }

    void loadMessages(selectedThreadId, providerId)
      .then((data) => {
        if (!active) {
          return;
        }
        setMessages(data);
      })
      .catch((loadError: unknown) => {
        if (!active) {
          return;
        }
        const message =
          loadError instanceof Error ? loadError.message : String(loadError);
        setError(message);
      })
      .finally(() => {
        if (active) {
          setLoadingMessages(false);
        }
      });

    return () => {
      active = false;
    };
  }, [loadMessages, rightPaneMode, selectedThread, selectedThreadId]);

  const handleSendMessage = useCallback(
    async (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      if (!selectedThread || sending) {
        return;
      }

      const content = draft.trim();
      if (!content) {
        return;
      }

      const optimisticMessage = makeLocalMessage("user", content);
      if (selectedThread.providerId !== "claude_code") {
        setError(
          "UI send is currently supported for Claude Code only. Use Terminal mode for Codex/OpenCode.",
        );
        return;
      }

      setMessages((current) => [...current, optimisticMessage]);
      setDraft("");
      setSending(true);
      setError(null);

      try {
        const response = await invoke<SendClaudeMessageResponse>("send_claude_message", {
          request: {
            threadId: selectedThread.id,
            content,
            projectPath: selectedThread.projectPath,
          },
        });

        const refreshed = await loadMessages(selectedThread.id, selectedThread.providerId);
        const hasResponse = refreshed.some(
          (message) =>
            message.role === "assistant" &&
            message.content.includes(response.responseText),
        );

        if (hasResponse) {
          setMessages(refreshed);
        } else {
          setMessages([
            ...refreshed,
            makeLocalMessage("assistant", response.responseText || response.rawOutput),
          ]);
        }
      } catch (sendError) {
        setMessages((current) => current.filter((item) => item !== optimisticMessage));
        const message =
          sendError instanceof Error ? sendError.message : String(sendError);
        setError(message);
      } finally {
        setSending(false);
      }
    },
    [draft, loadMessages, selectedThread, sending],
  );

  const handleComposerKeyDown = useCallback(
    (event: KeyboardEvent<HTMLTextAreaElement>) => {
      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
        event.preventDefault();
        event.currentTarget.form?.requestSubmit();
      }
    },
    [],
  );

  const handleSidebarResizeStart = useCallback(
    (event: ReactMouseEvent<HTMLDivElement>) => {
      if (sidebarCollapsed) {
        return;
      }
      resizeStateRef.current = {
        startX: event.clientX,
        startWidth: sidebarWidth,
      };
      setIsResizingSidebar(true);
      event.preventDefault();
    },
    [sidebarCollapsed, sidebarWidth],
  );

  const toggleSidebar = useCallback(() => {
    setSidebarCollapsed((current) => {
      if (current) {
        setSidebarWidth((prev) => clampSidebarWidth(prev));
      }
      return !current;
    });
  }, [clampSidebarWidth]);

  // Set up window drag region using native event listeners
  // This is a workaround for Tauri 2.x macOS overlay titlebar issue
  // See: https://github.com/tauri-apps/tauri/issues/9503
  useEffect(() => {
    const dragRegion = dragRegionRef.current;
    if (!dragRegion) {
      return;
    }

    const handleMouseDown = async (event: MouseEvent) => {
      // Only allow left mouse button
      if (event.button !== 0) {
        return;
      }
      // Start dragging the window
      void appWindow.startDragging();
    };

    dragRegion.addEventListener("mousedown", handleMouseDown);
    return () => {
      dragRegion.removeEventListener("mousedown", handleMouseDown);
    };
  }, [appWindow]);

  useEffect(() => {
    if (!isResizingSidebar) {
      return;
    }

    const handleMouseMove = (event: MouseEvent) => {
      const resizeState = resizeStateRef.current;
      if (!resizeState) {
        return;
      }
      const deltaX = event.clientX - resizeState.startX;
      const nextWidth = clampSidebarWidth(resizeState.startWidth + deltaX);
      setSidebarWidth(nextWidth);
    };

    const handleMouseUp = () => {
      resizeStateRef.current = null;
      setIsResizingSidebar(false);
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
  }, [clampSidebarWidth, isResizingSidebar]);

  return (
    <main className="relative h-full min-h-0 overflow-hidden bg-background">
      {/* Drag region for window movement - workaround for Tauri 2.x macOS overlay issue */}
      <div
        ref={dragRegionRef}
        data-window-drag-region="true"
        className="absolute left-0 right-0 top-0 z-[9999] select-none"
        style={{ height: WINDOW_DRAG_STRIP_HEIGHT }}
      />
      <section
        ref={layoutRef}
        className="grid h-full min-h-0 flex-1 overflow-hidden"
        style={layoutGridStyle}
      >
        {!sidebarCollapsed ? (
          <Card className="flex min-h-0 flex-col rounded-none border-0 bg-card/92 shadow-none pt-8">
            <CardHeader className="px-4 py-3 pb-2.5">
              <CardDescription className="text-[11px] uppercase tracking-[0.18em] text-primary/90">
                AgentDock
              </CardDescription>
              <CardTitle className="text-[22px] leading-none">Agent Console</CardTitle>
              <CardDescription className="text-xs">Thread switch + session relay</CardDescription>
              <div className="flex items-center gap-2 pt-1">
                <Button variant="secondary" size="sm" className="h-7 px-2.5 text-xs" disabled>
                  New Thread
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7 px-2.5 text-xs"
                  onClick={() => void loadThreads()}
                  disabled={loadingThreads}
                >
                  <RefreshCw className="mr-1.5 h-3 w-3" />
                  Refresh
                </Button>
              </div>
            </CardHeader>
            <CardContent className="flex min-h-0 flex-1 flex-col py-2.5 pl-2.5 pr-0">
              <div className="mb-1.5 flex items-center justify-between px-0.5">
                <p className="text-xs font-semibold uppercase tracking-[0.14em] text-muted-foreground">
                  Folders
                </p>
                <Badge variant="secondary" className="h-5 px-2 text-[10px]">
                  {folderGroups.length}
                </Badge>
              </div>

              {loadingThreads ? (
                <p className="px-1.5 py-2 text-xs text-muted-foreground">
                  Loading threads...
                </p>
              ) : threads.length === 0 ? (
                <p className="px-1.5 py-2 text-xs text-muted-foreground">
                  No sessions found in <code>~/.claude/projects</code>, <code>~/.codex/sessions</code>, or{" "}
                  <code>~/.local/share/opencode/storage/session</code>.
                </p>
              ) : (
                <div className="min-h-0 flex-1 overflow-y-auto">
                  <style>{`
                    .sidebar-scroll::-webkit-scrollbar {
                      width: 6px;
                    }
                    .sidebar-scroll::-webkit-scrollbar-track {
                      background: transparent;
                      border: none;
                    }
                    .sidebar-scroll::-webkit-scrollbar-thumb {
                      background: hsl(var(--muted-foreground) / 0.4);
                      border-radius: 3px;
                    }
                    .sidebar-scroll::-webkit-scrollbar-thumb:hover {
                      background: hsl(var(--muted-foreground) / 0.6);
                    }
                  `}</style>
                  <div className="sidebar-scroll h-full">
                  <ul className="w-full space-y-2 pb-1.5">
                    {folderGroups.map((group) => (
                      <ThreadFolderGroup
                        key={group.key}
                        group={group}
                        isActiveFolder={group.key === selectedFolderKey}
                        selectedThreadId={selectedThreadId}
                        onSelectThread={handleSelectThread}
                        onCreateThread={handleCreateThreadInFolder}
                        isCreatingThread={creatingThreadFolderKey === group.key}
                        formatLastActive={formatLastActive}
                        getPreview={threadPreview}
                      />
                    ))}
                  </ul>
                  </div>
                </div>
              )}
            </CardContent>
          </Card>
        ) : null}

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
                isResizingSidebar ? "bg-primary/55" : "group-hover:bg-primary/45",
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
          <CardHeader className="px-4 py-3 pb-2.5">
            <div className="flex flex-wrap items-start justify-between gap-3">
              <div className="space-y-1">
                <p
                  className={cn(
                    "inline-flex items-center gap-1.5 text-[11px] uppercase tracking-[0.16em]",
                    headerProviderAccent,
                  )}
                >
                  {isCodexProvider(headerProviderId) ? (
                    <SquareTerminal className="h-3.5 w-3.5" />
                  ) : isOpenCodeProvider(headerProviderId) ? (
                    <Code2 className="h-3.5 w-3.5" />
                  ) : (
                    <Bot className="h-3.5 w-3.5" />
                  )}
                  {headerProviderName}
                </p>
                <CardTitle className="text-[22px] leading-none">{headerTitle}</CardTitle>
                <CardDescription className="truncate text-xs">
                  {headerProjectPath}
                </CardDescription>
              </div>

              <div className="grid justify-items-end gap-1 text-xs text-muted-foreground">
                <span>
                  {rightPaneMode === "terminal"
                    ? "Embedded terminal"
                    : loadingMessages
                      ? "Syncing..."
                      : "Ready"}
                </span>
                <div className="flex items-center gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="icon"
                    className="h-7 w-7 rounded-md"
                    onClick={toggleSidebar}
                  >
                    {sidebarCollapsed ? (
                      <PanelLeftOpen className="h-3.5 w-3.5" />
                    ) : (
                      <PanelLeftClose className="h-3.5 w-3.5" />
                    )}
                  </Button>
                  <div className="inline-flex items-center rounded-md border border-border/70 bg-muted/35 p-0.5">
                    <Button
                      type="button"
                      variant={rightPaneMode === "terminal" ? "secondary" : "ghost"}
                      size="sm"
                      className="h-6 gap-1 px-2 text-[11px]"
                      onClick={() => setRightPaneMode("terminal")}
                    >
                      <SquareTerminal className="h-3 w-3" />
                      Terminal
                    </Button>
                    <Button
                      type="button"
                      variant={rightPaneMode === "ui" ? "secondary" : "ghost"}
                      size="sm"
                      className="h-6 gap-1 px-2 text-[11px]"
                      onClick={() => setRightPaneMode("ui")}
                    >
                      <Monitor className="h-3 w-3" />
                      UI
                    </Button>
                  </div>
                  {rightPaneMode === "ui" ? (
                    <>
                      <Switch
                        id="show-tool-events"
                        checked={showToolEvents}
                        onCheckedChange={setShowToolEvents}
                      />
                      <label htmlFor="show-tool-events" className="cursor-pointer">
                        Show steps
                      </label>
                      {toolCount > 0 ? (
                        <Badge variant="outline" className="h-5 px-2 text-[10px]">
                          {toolCount}
                        </Badge>
                      ) : null}
                    </>
                  ) : null}
                </div>
                {rightPaneMode === "ui" ? (
                  <span>
                    {displayedMessages.length}/{messages.length} messages
                  </span>
                ) : (
                  <span className="text-[11px]">Interactive CLI in-app</span>
                )}
              </div>
            </div>
          </CardHeader>
          <Separator />

          <CardContent
            className={cn(
              "min-h-0 flex-1",
              "p-0",
            )}
          >
            {rightPaneMode === "terminal" ? (
              <div className="h-full w-full">
                <EmbeddedTerminal
                  thread={
                    newThreadLaunch
                      ? {
                          id: `__new__:${newThreadLaunch.launchId}`,
                          providerId: newThreadLaunch.providerId,
                          projectPath: newThreadLaunch.projectPath,
                        }
                      : selectedThread
                      ? {
                          id: selectedThread.id,
                          providerId: selectedThread.providerId,
                          projectPath: selectedThread.projectPath,
                        }
                      : null
                  }
                  launchRequest={newThreadLaunch}
                  onLaunchRequestSettled={handleNewThreadLaunchSettled}
                  onError={setError}
                />
              </div>
            ) : (
              <ScrollArea className="h-full px-5 py-3">
                {error ? (
                  <div className="mb-3 rounded-md border border-destructive/40 bg-destructive/10 px-2.5 py-1.5 text-xs text-destructive">
                    {error}
                  </div>
                ) : null}
                {selectedThread && !canUseUiComposer ? (
                  <div className="mb-3 rounded-md border border-primary/30 bg-primary/5 px-2.5 py-1.5 text-xs text-muted-foreground">
                    Non-Claude threads are read-only in UI mode for now. Use Terminal mode to continue chatting.
                  </div>
                ) : null}

                {!selectedThread ? (
                  <p className="text-sm text-muted-foreground">
                    Choose a thread from the left panel.
                  </p>
                ) : loadingMessages ? (
                  <p className="text-sm text-muted-foreground">
                    Loading conversation messages...
                  </p>
                ) : displayedMessages.length === 0 ? (
                  <p className="text-sm text-muted-foreground">
                    No visible messages in this thread.
                  </p>
                ) : (
                  <ul className="space-y-1.5">
                    {displayedMessages.map((message, index) => {
                      const isTool = message.kind === "tool";
                      const isUser = message.role === "user";
                      const isLast = index === displayedMessages.length - 1;
                      const toolParts = isTool
                        ? splitToolMessage(message.content)
                        : { headline: "", detail: undefined };
                      const toolTitle = isTool
                        ? parseToolTitle(toolParts.headline)
                        : { strong: "", rest: undefined };

                      return (
                        <li
                          key={`${message.role}-${message.kind}-${message.timestampMs ?? index}-${index}`}
                          className="relative pl-8"
                        >
                          {!isLast ? (
                            <span className="absolute left-[8px] top-3.5 h-[calc(100%+0.42rem)] w-px bg-border/75" />
                          ) : null}
                          <span
                            className={cn(
                              "absolute left-[4px] top-2 h-2.5 w-2.5 rounded-full border border-card shadow-sm",
                              isTool
                                ? "bg-emerald-500"
                                : isUser
                                  ? "bg-primary/70"
                                  : "bg-slate-500/70",
                            )}
                          />

                          {isTool ? (
                            <article className="space-y-0.5 pb-0.5">
                              <p className="whitespace-pre-wrap break-words text-[14px] leading-relaxed text-foreground">
                                <span className="font-semibold">{toolTitle.strong}</span>
                                {toolTitle.rest ? (
                                  <span className="text-muted-foreground">
                                    {" "}
                                    {toolTitle.rest}
                                  </span>
                                ) : null}
                              </p>
                              {toolParts.detail ? (
                                <p className="whitespace-pre-wrap break-words text-[14px] leading-relaxed text-muted-foreground">
                                  {toolParts.detail}
                                </p>
                              ) : null}
                              {toolParts.ioLabel ? (
                                <div className="mt-1 overflow-hidden rounded-md border border-border/80 bg-muted/35">
                                  <div className="border-b border-border/80 px-2 py-1 text-[11px] font-semibold tracking-wide text-muted-foreground">
                                    {toolParts.ioLabel}
                                  </div>
                                  <pre className="max-h-48 overflow-auto px-2 py-1.5 font-mono text-[12px] leading-relaxed text-foreground">
                                    {normalizeCodeBody(toolParts.ioBody ?? "")}
                                  </pre>
                                </div>
                              ) : null}
                            </article>
                          ) : (
                            <div className="space-y-0.5 pb-0.5">
                              {isUser ? (
                                <article className="inline-flex max-w-[84%] rounded-lg border border-border bg-secondary/70 px-2.5 py-1 shadow-sm">
                                  <p className="whitespace-pre-wrap break-words text-[13px] leading-relaxed">
                                    {message.content}
                                  </p>
                                </article>
                              ) : (
                                <p className="whitespace-pre-wrap break-words text-[13px] leading-relaxed text-foreground">
                                  {message.content}
                                </p>
                              )}
                            </div>
                          )}
                        </li>
                      );
                    })}
                  </ul>
                )}
              </ScrollArea>
            )}
          </CardContent>

          {rightPaneMode === "ui" ? (
            <CardFooter className="p-2 pt-1">
              <form onSubmit={handleSendMessage} className="w-full">
                <div className="mx-auto w-full max-w-[980px] rounded-xl border border-primary/25 bg-card shadow-sm">
                  <Textarea
                    value={draft}
                    placeholder={
                      !selectedThread
                        ? "Select a thread first"
                        : !canUseUiComposer
                          ? "Switch to Terminal mode for Codex input"
                          : "⌘ Esc to focus or unfocus Claude"
                    }
                    onChange={(event) => setDraft(event.target.value)}
                    onKeyDown={handleComposerKeyDown}
                    disabled={!selectedThread || sending || !canUseUiComposer}
                    rows={2}
                    className={cn(
                      "min-h-[64px] max-h-[180px] resize-none border-0 bg-transparent px-3 py-2.5 text-[13px] placeholder:text-muted-foreground/70 focus-visible:ring-0 focus-visible:ring-offset-0",
                      "text-foreground",
                    )}
                  />
                  <Separator />
                  <div className="flex items-center justify-between gap-2 bg-muted/35 px-2.5 py-2 text-muted-foreground">
                    <div className="flex min-w-0 items-center gap-1.5">
                      <span className="inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[12px] text-muted-foreground">
                        <Pencil className="h-3.5 w-3.5" />
                        Ask before edits
                      </span>
                      <span className="inline-flex min-w-0 max-w-[240px] items-center gap-1 rounded-md px-1.5 py-0.5 text-[12px] text-muted-foreground">
                        <span className="font-mono text-[11px]">&lt;/&gt;</span>
                        <span className="truncate">
                          {selectedThread ? selectedThread.title : "thread"}
                        </span>
                      </span>
                    </div>
                    <div className="flex items-center gap-1.5">
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        disabled={!selectedThread || sending || !canUseUiComposer}
                        className="h-7 w-7 rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
                      >
                        <Paperclip className="h-3.5 w-3.5" />
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        disabled={!selectedThread || sending || !canUseUiComposer}
                        className="h-7 w-7 rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
                      >
                        <Slash className="h-3.5 w-3.5" />
                      </Button>
                      <Button
                        type="submit"
                        size="icon"
                        disabled={!selectedThread || sending || !draft.trim() || !canUseUiComposer}
                        className="h-8 w-8 rounded-lg border border-primary/40 bg-primary text-primary-foreground hover:bg-primary/90"
                      >
                        <ArrowUp className="h-3.5 w-3.5" />
                      </Button>
                    </div>
                  </div>
                </div>
              </form>
            </CardFooter>
          ) : null}
        </Card>
      </section>
    </main>
  );
}

export default App;
