import { invoke } from "@tauri-apps/api/core";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { RefObject } from "react";

import type { TerminalProviderHelpDoc } from "@/components/terminal/helpDocs";
import { TERMINAL_PROVIDER_HELP_DOCS } from "@/components/terminal/helpDocs";
import { TERMINAL_THEMES, type TerminalVisualTheme } from "@/components/terminal/theme";
import { isSupportedProvider } from "@/lib/provider";
import type { TerminalTheme, ThreadProviderId } from "@/types";

import type {
  EmbeddedTerminalNewThreadLaunch,
  EmbeddedTerminalThread,
  SessionLaunchTarget,
  TerminalSessionState,
  ThreadRuntimeState,
} from "./types";
import { useTerminalHostEffects } from "./useTerminalHostEffects";
import { useTerminalSessionLifecycle } from "./useTerminalSessionLifecycle";

interface UseEmbeddedTerminalControllerProps {
  thread: EmbeddedTerminalThread | null;
  terminalTheme: TerminalTheme;
  launchRequest?: EmbeddedTerminalNewThreadLaunch | null;
  onLaunchRequestSettled?: (launch: EmbeddedTerminalNewThreadLaunch) => void;
  onError?: (message: string | null) => void;
}

interface UseEmbeddedTerminalControllerResult {
  hostRef: RefObject<HTMLDivElement | null>;
  helpButtonRef: RefObject<HTMLButtonElement | null>;
  helpPopoverRef: RefObject<HTMLDivElement | null>;
  activeTheme: TerminalVisualTheme;
  activeProviderId: ThreadProviderId | null;
  activeProviderHelpDoc: TerminalProviderHelpDoc | null;
  isSwitchingThread: boolean;
  starting: boolean;
  isRefreshing: boolean;
  refreshError: string | null;
  lastCommand: string | null;
  handleRefreshSession: () => void;
}

const SESSION_BUFFER_MAX_CHARS = 800_000;

export function useEmbeddedTerminalController({
  thread,
  terminalTheme,
  launchRequest,
  onLaunchRequestSettled,
  onError,
}: UseEmbeddedTerminalControllerProps): UseEmbeddedTerminalControllerResult {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const sessionIdRef = useRef<string | null>(null);
  const sessionsByThreadRef = useRef<Map<string, TerminalSessionState>>(new Map());
  const sessionsByIdRef = useRef<Map<string, TerminalSessionState>>(new Map());
  const cleanupCheckInFlightRef = useRef<Set<string>>(new Set());
  const resizeFrameRef = useRef<number | null>(null);
  const pendingResizeRef = useRef<{ cols: number; rows: number } | null>(null);
  const helpButtonRef = useRef<HTMLButtonElement | null>(null);
  const helpPopoverRef = useRef<HTMLDivElement | null>(null);
  const lastHandledRefreshRequestRef = useRef(0);
  const [isSwitchingThread, setIsSwitchingThread] = useState(false);
  const [starting, setStarting] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [refreshRequestId, setRefreshRequestId] = useState(0);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const [lastCommand, setLastCommand] = useState<string | null>(null);
  const initialThemeRef = useRef(TERMINAL_THEMES[terminalTheme]);
  const activeTheme = TERMINAL_THEMES[terminalTheme];
  const threadId = thread?.id ?? null;
  const threadProviderId = thread?.providerId ?? null;
  const threadProjectPath = thread?.projectPath ?? null;
  const launchRequestId = launchRequest?.launchId ?? null;
  const launchRequestProviderId = launchRequest?.providerId ?? null;
  const launchRequestProjectPath = launchRequest?.projectPath ?? null;
  const launchRequestKnownThreadIds = launchRequest?.knownThreadIds ?? null;

  const threadKey = useMemo(() => {
    if (!threadId || !threadProviderId || !threadProjectPath) {
      return null;
    }
    return `${threadProviderId}:${threadId}:${threadProjectPath}`;
  }, [threadId, threadProjectPath, threadProviderId]);

  const launchTarget = useMemo<SessionLaunchTarget | null>(() => {
    if (
      launchRequestId !== null &&
      launchRequestProviderId &&
      launchRequestProjectPath &&
      launchRequestKnownThreadIds
    ) {
      return {
        mode: "new",
        key: `new:${launchRequestId}`,
        launchId: launchRequestId,
        providerId: launchRequestProviderId,
        projectPath: launchRequestProjectPath,
        knownThreadIds: launchRequestKnownThreadIds,
      };
    }
    if (!threadId || !threadProviderId || !threadProjectPath || !threadKey) {
      return null;
    }
    return {
      mode: "resume",
      key: threadKey,
      threadId,
      providerId: threadProviderId,
      projectPath: threadProjectPath,
    };
  }, [
    launchRequestId,
    launchRequestKnownThreadIds,
    launchRequestProjectPath,
    launchRequestProviderId,
    threadId,
    threadKey,
    threadProjectPath,
    threadProviderId,
  ]);

  const activeProviderId = useMemo<ThreadProviderId | null>(() => {
    const providerId = launchTarget?.providerId ?? thread?.providerId;
    if (!providerId || !isSupportedProvider(providerId)) {
      return null;
    }
    return providerId;
  }, [launchTarget?.providerId, thread?.providerId]);

  const activeProviderHelpDoc = useMemo(() => {
    if (!activeProviderId) {
      return null;
    }
    return TERMINAL_PROVIDER_HELP_DOCS[activeProviderId];
  }, [activeProviderId]);

  useEffect(() => {
    setRefreshError(null);
  }, [launchTarget?.key]);

  const handleRefreshSession = useCallback(() => {
    if (isRefreshing || starting || isSwitchingThread) {
      return;
    }
    if (!launchTarget) {
      setRefreshError("Select a thread before refreshing the terminal session.");
      return;
    }
    setRefreshError(null);
    setIsRefreshing(true);
    setRefreshRequestId((value) => value + 1);
  }, [isRefreshing, isSwitchingThread, launchTarget, starting]);

  const queueRemoteResize = useCallback(
    (cols: number, rows: number) => {
      pendingResizeRef.current = {
        cols: Math.max(40, cols || 120),
        rows: Math.max(12, rows || 36),
      };

      if (resizeFrameRef.current !== null) {
        return;
      }

      resizeFrameRef.current = window.requestAnimationFrame(() => {
        resizeFrameRef.current = null;
        const size = pendingResizeRef.current;
        const sessionId = sessionIdRef.current;
        if (!size || !sessionId) {
          return;
        }

        void invoke("resize_embedded_terminal", {
          request: {
            sessionId,
            cols: size.cols,
            rows: size.rows,
          },
        }).catch((error) => {
          const message = error instanceof Error ? error.message : String(error);
          onError?.(message);
        });
      });
    },
    [onError],
  );

  const appendSessionBuffer = useCallback((session: TerminalSessionState, chunk: string) => {
    if (!chunk) {
      return;
    }
    session.lastTouchedAt = Date.now();
    session.buffer += chunk;
    if (session.buffer.length > SESSION_BUFFER_MAX_CHARS) {
      session.buffer = session.buffer.slice(-SESSION_BUFFER_MAX_CHARS);
    }
  }, []);

  const closeSessionById = useCallback(async (sessionId: string) => {
    const session = sessionsByIdRef.current.get(sessionId);
    cleanupCheckInFlightRef.current.delete(sessionId);
    if (!session) {
      return;
    }

    sessionsByIdRef.current.delete(sessionId);
    const threadSession = sessionsByThreadRef.current.get(session.threadKey);
    if (threadSession?.sessionId === sessionId) {
      sessionsByThreadRef.current.delete(session.threadKey);
    }
    if (sessionIdRef.current === sessionId) {
      sessionIdRef.current = null;
    }

    try {
      await invoke("close_embedded_terminal", {
        request: {
          sessionId,
        },
      });
    } catch {
      // Session may already be closed by backend.
    }
  }, []);

  const cleanupDormantSessions = useCallback(
    (activeThreadKey: string | null) => {
      for (const session of sessionsByIdRef.current.values()) {
        if (!session.running) {
          continue;
        }
        if (session.threadKey === activeThreadKey) {
          continue;
        }
        if (cleanupCheckInFlightRef.current.has(session.sessionId)) {
          continue;
        }
        if (!session.runtimeThreadId) {
          void closeSessionById(session.sessionId);
          continue;
        }
        if (session.hasUserInput) {
          continue;
        }

        if (
          session.providerId !== "codex" &&
          session.providerId !== "claude_code" &&
          session.providerId !== "opencode"
        ) {
          void closeSessionById(session.sessionId);
          continue;
        }

        cleanupCheckInFlightRef.current.add(session.sessionId);
        const sessionId = session.sessionId;
        const threadId = session.runtimeThreadId;
        const providerId = session.providerId;
        void (async () => {
          try {
            const command =
              providerId === "claude_code"
                ? "get_claude_thread_runtime_state"
                : providerId === "codex"
                  ? "get_codex_thread_runtime_state"
                  : "get_opencode_thread_runtime_state";
            const state = await invoke<ThreadRuntimeState>(command, {
              request: { threadId },
            });
            const current = sessionsByIdRef.current.get(sessionId);
            if (!current || current.threadKey === activeThreadKey) {
              return;
            }
            if (!current.hasUserInput && !state.agentAnswering) {
              await closeSessionById(sessionId);
            }
          } catch {
            // If runtime status cannot be determined, fallback to keeping session.
          } finally {
            cleanupCheckInFlightRef.current.delete(sessionId);
          }
        })();
      }
    },
    [closeSessionById],
  );

  const closeAllSessions = useCallback(async () => {
    const sessionIds = [...sessionsByIdRef.current.keys()];
    sessionIdRef.current = null;
    sessionsByThreadRef.current.clear();
    sessionsByIdRef.current.clear();
    cleanupCheckInFlightRef.current.clear();
    await Promise.all(
      sessionIds.map(async (sessionId) => {
        try {
          await invoke("close_embedded_terminal", {
            request: {
              sessionId,
            },
          });
        } catch {
          // Session may have already exited; safe to ignore.
        }
      }),
    );
  }, []);

  const writeInputToSession = useCallback(
    (data: string) => {
      const sessionId = sessionIdRef.current;
      if (!sessionId) {
        return;
      }
      const session = sessionsByIdRef.current.get(sessionId);
      if (session) {
        session.hasUserInput = true;
        session.lastTouchedAt = Date.now();
      }

      void invoke("write_embedded_terminal_input", {
        request: {
          sessionId,
          data,
        },
      }).catch((error) => {
        const message = error instanceof Error ? error.message : String(error);
        onError?.(message);
      });
    },
    [onError],
  );

  const tuneHelperTextarea = useCallback(() => {
    const host = hostRef.current;
    if (!host) {
      return;
    }

    const textarea = host.querySelector(
      ".xterm-helper-textarea",
    ) as HTMLTextAreaElement | null;
    if (!textarea) {
      return;
    }

    // Keep xterm helper textarea inside viewport (still near-invisible)
    // so macOS dictation/IME can target it more reliably.
    // Keep xterm-controlled position (caret-following), but make it focusable.
    textarea.style.width = "12px";
    textarea.style.height = "20px";
    textarea.style.opacity = "0.001";
    textarea.style.zIndex = "8";
    textarea.style.color = "transparent";
    textarea.style.background = "transparent";
    textarea.style.caretColor = "transparent";
    textarea.style.border = "0";
    textarea.readOnly = false;
    textarea.disabled = false;
    textarea.autocomplete = "off";
    textarea.autocapitalize = "off";
    textarea.spellcheck = false;
    textarea.setAttribute("autocorrect", "off");
  }, []);

  useTerminalHostEffects({
    hostRef,
    terminalRef,
    fitAddonRef,
    sessionIdRef,
    sessionsByThreadRef,
    sessionsByIdRef,
    resizeFrameRef,
    activeTheme,
    initialThemeRef,
    appendSessionBuffer,
    closeAllSessions,
    queueRemoteResize,
    tuneHelperTextarea,
    writeInputToSession,
  });

  useTerminalSessionLifecycle({
    appendSessionBuffer,
    closeSessionById,
    cleanupDormantSessions,
    fitAddonRef,
    launchTarget,
    onError,
    onLaunchRequestSettled,
    queueRemoteResize,
    refreshRequestId,
    sessionIdRef,
    sessionsByThreadRef,
    sessionsByIdRef,
    setIsRefreshing,
    setIsSwitchingThread,
    setLastCommand,
    setRefreshError,
    setStarting,
    terminalRef,
    terminalTheme,
    lastHandledRefreshRequestRef,
  });

  return {
    hostRef,
    helpButtonRef,
    helpPopoverRef,
    activeTheme,
    activeProviderId,
    activeProviderHelpDoc,
    isSwitchingThread,
    starting,
    isRefreshing,
    refreshError,
    lastCommand,
    handleRefreshSession,
  };
}
