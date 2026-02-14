import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type {
  AgentThreadMessage,
  AgentThreadSummary,
  RightPaneMode,
  ThreadProviderId,
} from "@/types";
import {
  folderNameFromProjectPath,
  makeLocalMessage,
  normalizeProjectPath,
  pickCreatedThread,
  resolveSelectedThreadId,
  sortableTimestamp,
} from "@/lib/thread";

const RIGHT_PANE_MODE_KEY = "agentdock.desktop.right_pane_mode";

function readStoredRightPaneMode(): RightPaneMode {
  if (typeof window === "undefined") {
    return "terminal";
  }
  const raw = window.localStorage.getItem(RIGHT_PANE_MODE_KEY);
  return raw === "ui" ? "ui" : "terminal";
}

export interface ThreadFolderGroupItem<T extends AgentThreadSummary = AgentThreadSummary> {
  key: string;
  folderName: string;
  threads: T[];
}

export interface EmbeddedTerminalNewThreadLaunch {
  launchId: number;
  providerId: string;
  projectPath: string;
  knownThreadIds: string[];
}

export interface EmbeddedTerminalLaunchSettledPayload {
  launch: EmbeddedTerminalNewThreadLaunch;
  started: boolean;
}

export type NewThreadBindingStatus = "starting" | "awaiting_discovery";

export interface UseThreadsResult {
  threads: AgentThreadSummary[];
  selectedThreadId: string | null;
  selectedThread: AgentThreadSummary | null;
  messages: AgentThreadMessage[];
  folderGroups: ThreadFolderGroupItem[];
  selectedFolderKey: string | null;
  loadingThreads: boolean;
  loadingMessages: boolean;
  sending: boolean;
  error: string | null;
  rightPaneMode: RightPaneMode;
  creatingThreadFolderKey: string | null;
  newThreadLaunch: EmbeddedTerminalNewThreadLaunch | null;
  newThreadBindingStatus: NewThreadBindingStatus | null;
  setRightPaneMode: (mode: RightPaneMode) => void;
  setError: (error: string | null) => void;
  loadThreads: () => Promise<void>;
  loadMessages: (threadId: string, providerId: string) => Promise<AgentThreadMessage[]>;
  handleSelectThread: (threadId: string) => void;
  handleCreateThreadInFolder: (projectPath: string, providerId: ThreadProviderId) => Promise<void>;
  handleNewThreadLaunchSettled: (payload: EmbeddedTerminalLaunchSettledPayload) => void;
  handleEmbeddedTerminalSessionExit: () => void;
  handleSendMessage: (content: string) => Promise<void>;
}

export function useThreads(): UseThreadsResult {
  const [threads, setThreads] = useState<AgentThreadSummary[]>([]);
  const [selectedThreadId, setSelectedThreadId] = useState<string | null>(null);
  const [messages, setMessages] = useState<AgentThreadMessage[]>([]);
  const [loadingThreads, setLoadingThreads] = useState(true);
  const [loadingMessages, setLoadingMessages] = useState(false);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [rightPaneMode, setRightPaneMode] = useState<RightPaneMode>(readStoredRightPaneMode);
  const [creatingThreadFolderKey, setCreatingThreadFolderKey] = useState<string | null>(null);
  const [newThreadLaunch, setNewThreadLaunch] = useState<EmbeddedTerminalNewThreadLaunch | null>(
    null,
  );
  const [newThreadBindingStatus, setNewThreadBindingStatus] =
    useState<NewThreadBindingStatus | null>(null);
  const pendingNewThreadLaunchIdRef = useRef<number | null>(null);

  const selectedThread = useMemo(
    () => threads.find((thread) => thread.id === selectedThreadId) ?? null,
    [threads, selectedThreadId],
  );

  const folderGroups = useMemo<ThreadFolderGroupItem[]>(() => {
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

  const clearPendingNewThreadLaunch = useCallback((launch: EmbeddedTerminalNewThreadLaunch) => {
    const isActiveLaunch = pendingNewThreadLaunchIdRef.current === launch.launchId;
    if (!isActiveLaunch) {
      return false;
    }

    pendingNewThreadLaunchIdRef.current = null;
    setNewThreadBindingStatus(null);
    setCreatingThreadFolderKey((current) =>
      current === launch.projectPath ? null : current,
    );
    setNewThreadLaunch((current) =>
      current?.launchId === launch.launchId ? null : current,
    );
    return true;
  }, []);

  const tryBindNewThreadLaunch = useCallback(
    async (launch: EmbeddedTerminalNewThreadLaunch) => {
      if (pendingNewThreadLaunchIdRef.current !== launch.launchId) {
        return false;
      }

      const data = await invoke<AgentThreadSummary[]>("list_threads");
      if (pendingNewThreadLaunchIdRef.current !== launch.launchId) {
        return false;
      }

      setThreads(data);
      const createdThread = pickCreatedThread(data, launch);
      if (!createdThread) {
        return false;
      }

      setSelectedThreadId(createdThread.id);
      clearPendingNewThreadLaunch(launch);
      return true;
    },
    [clearPendingNewThreadLaunch],
  );

  const handleCreateThreadInFolder = useCallback(
    async (projectPath: string, providerId: ThreadProviderId) => {
      const launchId = Date.now();
      setCreatingThreadFolderKey(projectPath);
      setError(null);
      setRightPaneMode("terminal");
      pendingNewThreadLaunchIdRef.current = launchId;
      setNewThreadBindingStatus("starting");
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
    setNewThreadBindingStatus(null);
    setCreatingThreadFolderKey(null);
    setNewThreadLaunch(null);
    setSelectedThreadId(threadId);
  }, []);

  const handleNewThreadLaunchSettled = useCallback(
    ({ launch, started }: EmbeddedTerminalLaunchSettledPayload) => {
      if (pendingNewThreadLaunchIdRef.current !== launch.launchId) {
        return;
      }

      if (!started) {
        clearPendingNewThreadLaunch(launch);
        return;
      }

      setCreatingThreadFolderKey((current) =>
        current === launch.projectPath ? null : current,
      );
      void (async () => {
        try {
          const isBound = await tryBindNewThreadLaunch(launch);
          if (isBound) {
            return;
          }
          if (pendingNewThreadLaunchIdRef.current === launch.launchId) {
            setNewThreadBindingStatus("awaiting_discovery");
          }
        } catch {
          if (pendingNewThreadLaunchIdRef.current === launch.launchId) {
            setNewThreadBindingStatus("awaiting_discovery");
          }
        }
      })();
    },
    [clearPendingNewThreadLaunch, tryBindNewThreadLaunch],
  );

  const handleEmbeddedTerminalSessionExit = useCallback(() => {
    const activeLaunchId = pendingNewThreadLaunchIdRef.current;
    if (activeLaunchId === null) {
      return;
    }

    pendingNewThreadLaunchIdRef.current = null;
    setNewThreadBindingStatus(null);
    setCreatingThreadFolderKey(null);
    setNewThreadLaunch((current) =>
      current?.launchId === activeLaunchId ? null : current,
    );
    void loadThreads();
  }, [loadThreads]);

  const handleSendMessage = useCallback(
    async (content: string) => {
      if (!selectedThread || sending) {
        return;
      }

      const trimmedContent = content.trim();
      if (!trimmedContent) {
        return;
      }

      const optimisticMessage = makeLocalMessage("user", trimmedContent);
      
      if (selectedThread.providerId !== "claude_code") {
        setError(
          "UI send is currently supported for Claude Code only. Use Terminal mode for Codex/OpenCode.",
        );
        return;
      }

      setMessages((current) => [...current, optimisticMessage]);
      setSending(true);
      setError(null);

      try {
        interface SendClaudeMessageResponse {
          threadId: string;
          responseText: string;
          rawOutput: string;
        }

        const response = await invoke<SendClaudeMessageResponse>("send_claude_message", {
          request: {
            threadId: selectedThread.id,
            content: trimmedContent,
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
    [loadMessages, selectedThread, sending],
  );

  useEffect(() => {
    void loadThreads();
  }, [loadThreads]);

  useEffect(() => {
    if (rightPaneMode === "terminal") {
      return;
    }
    pendingNewThreadLaunchIdRef.current = null;
    setNewThreadBindingStatus(null);
    setNewThreadLaunch(null);
    setCreatingThreadFolderKey(null);
  }, [rightPaneMode]);

  useEffect(() => {
    if (
      rightPaneMode !== "terminal" ||
      newThreadBindingStatus !== "awaiting_discovery" ||
      !newThreadLaunch ||
      pendingNewThreadLaunchIdRef.current !== newThreadLaunch.launchId
    ) {
      return;
    }

    let cancelled = false;
    let timeoutId: number | null = null;

    const pollForCreatedThread = async () => {
      if (cancelled || pendingNewThreadLaunchIdRef.current !== newThreadLaunch.launchId) {
        return;
      }

      try {
        const isBound = await tryBindNewThreadLaunch(newThreadLaunch);
        if (isBound || cancelled) {
          return;
        }
      } catch {
        // Keep polling quietly to avoid noisy global errors.
      }

      if (cancelled || pendingNewThreadLaunchIdRef.current !== newThreadLaunch.launchId) {
        return;
      }

      timeoutId = window.setTimeout(() => {
        void pollForCreatedThread();
      }, 2000);
    };

    timeoutId = window.setTimeout(() => {
      void pollForCreatedThread();
    }, 2000);

    return () => {
      cancelled = true;
      if (timeoutId !== null) {
        window.clearTimeout(timeoutId);
      }
    };
  }, [newThreadBindingStatus, newThreadLaunch, rightPaneMode, tryBindNewThreadLaunch]);

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

  return {
    threads,
    selectedThreadId,
    selectedThread,
    messages,
    folderGroups,
    selectedFolderKey,
    loadingThreads,
    loadingMessages,
    sending,
    error,
    rightPaneMode,
    creatingThreadFolderKey,
    newThreadLaunch,
    newThreadBindingStatus,
    setRightPaneMode,
    setError,
    loadThreads,
    loadMessages,
    handleSelectThread,
    handleCreateThreadInFolder,
    handleNewThreadLaunchSettled,
    handleEmbeddedTerminalSessionExit,
    handleSendMessage,
  };
}
