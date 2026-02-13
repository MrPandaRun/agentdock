import { invoke } from "@tauri-apps/api/core";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import { useEffect } from "react";
import type {
  Dispatch,
  MutableRefObject,
  SetStateAction,
} from "react";

import type { TerminalTheme } from "@/types";

import type {
  EmbeddedTerminalNewThreadLaunch,
  SessionLaunchTarget,
  StartEmbeddedTerminalResponse,
  TerminalSessionState,
} from "./types";

interface UseTerminalSessionLifecycleProps {
  appendSessionBuffer: (session: TerminalSessionState, chunk: string) => void;
  closeSessionById: (sessionId: string) => Promise<void>;
  cleanupDormantSessions: (activeThreadKey: string | null) => void;
  fitAddonRef: MutableRefObject<FitAddon | null>;
  launchTarget: SessionLaunchTarget | null;
  onError?: (message: string | null) => void;
  onLaunchRequestSettled?: (launch: EmbeddedTerminalNewThreadLaunch) => void;
  queueRemoteResize: (cols: number, rows: number) => void;
  refreshRequestId: number;
  sessionIdRef: MutableRefObject<string | null>;
  sessionsByThreadRef: MutableRefObject<Map<string, TerminalSessionState>>;
  sessionsByIdRef: MutableRefObject<Map<string, TerminalSessionState>>;
  setIsRefreshing: Dispatch<SetStateAction<boolean>>;
  setIsSwitchingThread: Dispatch<SetStateAction<boolean>>;
  setLastCommand: Dispatch<SetStateAction<string | null>>;
  setRefreshError: Dispatch<SetStateAction<string | null>>;
  setStarting: Dispatch<SetStateAction<boolean>>;
  terminalRef: MutableRefObject<Terminal | null>;
  terminalTheme: TerminalTheme;
  lastHandledRefreshRequestRef: MutableRefObject<number>;
}

export function useTerminalSessionLifecycle({
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
}: UseTerminalSessionLifecycleProps) {
  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal) {
      return;
    }

    const forceRestart = refreshRequestId !== lastHandledRefreshRequestRef.current;
    if (forceRestart) {
      lastHandledRefreshRequestRef.current = refreshRequestId;
    }

    let cancelled = false;

    const startSession = async () => {
      setIsSwitchingThread(true);
      setStarting(false);
      if (forceRestart) {
        setRefreshError(null);
      }
      onError?.(null);
      setLastCommand(null);
      terminal.reset();
      terminal.clear();

      if (!launchTarget) {
        sessionIdRef.current = null;
        terminal.writeln("Select a thread from the left panel.");
        cleanupDormantSessions(null);
        if (forceRestart) {
          setIsRefreshing(false);
        }
        setStarting(false);
        setIsSwitchingThread(false);
        return;
      }

      if (launchTarget.mode === "resume") {
        for (const session of sessionsByIdRef.current.values()) {
          if (session.threadId !== launchTarget.threadId) {
            continue;
          }
          if (session.threadKey === launchTarget.key) {
            continue;
          }
          void closeSessionById(session.sessionId);
        }
      }

      const existing = sessionsByThreadRef.current.get(launchTarget.key);
      if (existing && !forceRestart) {
        const snapshot = existing.buffer;
        if (snapshot) {
          terminal.write(snapshot);
        }
        sessionIdRef.current = existing.sessionId;
        setLastCommand(existing.command);
        terminal.focus();
        queueRemoteResize(terminal.cols, terminal.rows);
        cleanupDormantSessions(launchTarget.key);
        setStarting(false);
        setIsSwitchingThread(false);
        return;
      }

      if (existing && forceRestart) {
        await closeSessionById(existing.sessionId);
      }

      setStarting(true);

      try {
        const response =
          launchTarget.mode === "resume"
            ? await invoke<StartEmbeddedTerminalResponse>("start_embedded_terminal", {
                request: {
                  threadId: launchTarget.threadId,
                  providerId: launchTarget.providerId,
                  projectPath: launchTarget.projectPath,
                  terminalTheme,
                  cols: Math.max(40, terminal.cols || 120),
                  rows: Math.max(12, terminal.rows || 36),
                },
              })
            : await invoke<StartEmbeddedTerminalResponse>("start_new_embedded_terminal", {
                request: {
                  providerId: launchTarget.providerId,
                  projectPath: launchTarget.projectPath,
                  terminalTheme,
                  cols: Math.max(40, terminal.cols || 120),
                  rows: Math.max(12, terminal.rows || 36),
                },
              });

        if (cancelled) {
          await invoke("close_embedded_terminal", {
            request: {
              sessionId: response.sessionId,
            },
          });
          return;
        }

        const session: TerminalSessionState = {
          threadKey: launchTarget.key,
          threadId: launchTarget.mode === "resume" ? launchTarget.threadId : null,
          runtimeThreadId: launchTarget.mode === "resume" ? launchTarget.threadId : null,
          providerId: launchTarget.providerId,
          sessionId: response.sessionId,
          command: response.command,
          buffer: "",
          running: true,
          hasUserInput: false,
          lastTouchedAt: Date.now(),
        };
        sessionsByThreadRef.current.set(launchTarget.key, session);
        sessionsByIdRef.current.set(response.sessionId, session);
        sessionIdRef.current = response.sessionId;
        setLastCommand(response.command);
        const launchBanner = `Launching: ${response.command}\r\n\r\n`;
        appendSessionBuffer(session, launchBanner);
        terminal.write(launchBanner);
        terminal.focus();
        queueRemoteResize(terminal.cols, terminal.rows);
        cleanupDormantSessions(launchTarget.key);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        if (forceRestart) {
          setRefreshError(message);
        }
        onError?.(message);
      } finally {
        if (launchTarget.mode === "new") {
          onLaunchRequestSettled?.({
            launchId: launchTarget.launchId,
            providerId: launchTarget.providerId,
            projectPath: launchTarget.projectPath,
            knownThreadIds: launchTarget.knownThreadIds,
          });
        }
        if (forceRestart) {
          setIsRefreshing(false);
        }
        if (cancelled) {
          return;
        }
        setStarting(false);
        setIsSwitchingThread(false);
        fitAddonRef.current?.fit();
      }
    };

    void startSession();

    return () => {
      cancelled = true;
    };
  }, [
    appendSessionBuffer,
    closeSessionById,
    cleanupDormantSessions,
    fitAddonRef,
    lastHandledRefreshRequestRef,
    launchTarget,
    onError,
    onLaunchRequestSettled,
    queueRemoteResize,
    refreshRequestId,
    sessionIdRef,
    sessionsByIdRef,
    sessionsByThreadRef,
    setIsRefreshing,
    setIsSwitchingThread,
    setLastCommand,
    setRefreshError,
    setStarting,
    terminalRef,
    terminalTheme,
  ]);
}
