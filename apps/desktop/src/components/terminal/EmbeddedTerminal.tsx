import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FitAddon } from "@xterm/addon-fit";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { Terminal } from "@xterm/xterm";
import { Loader2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import "@xterm/xterm/css/xterm.css";

export interface EmbeddedTerminalThread {
  id: string;
  providerId: string;
  projectPath: string;
}

export interface EmbeddedTerminalNewThreadLaunch {
  launchId: number;
  providerId: string;
  projectPath: string;
  knownThreadIds: string[];
}

interface StartEmbeddedTerminalResponse {
  sessionId: string;
  command: string;
}

interface EmbeddedTerminalOutputPayload {
  sessionId: string;
  data: string;
}

interface EmbeddedTerminalExitPayload {
  sessionId: string;
  statusCode?: number;
}

interface ThreadRuntimeState {
  agentAnswering: boolean;
  lastEventKind?: string | null;
  lastEventAtMs?: number | null;
}

interface EmbeddedTerminalProps {
  thread: EmbeddedTerminalThread | null;
  launchRequest?: EmbeddedTerminalNewThreadLaunch | null;
  onLaunchRequestSettled?: (launch: EmbeddedTerminalNewThreadLaunch) => void;
  onError?: (message: string | null) => void;
}

interface TerminalSessionState {
  threadKey: string;
  threadId: string | null;
  runtimeThreadId: string | null;
  providerId: string;
  sessionId: string;
  command: string;
  buffer: string;
  running: boolean;
  hasUserInput: boolean;
  lastTouchedAt: number;
}

const SESSION_BUFFER_MAX_CHARS = 800_000;

type SessionLaunchTarget =
  | {
      mode: "resume";
      key: string;
      threadId: string;
      providerId: string;
      projectPath: string;
    }
  | {
      mode: "new";
      key: string;
      launchId: number;
      providerId: string;
      projectPath: string;
      knownThreadIds: string[];
    };

export function EmbeddedTerminal({
  thread,
  launchRequest,
  onLaunchRequestSettled,
  onError,
}: EmbeddedTerminalProps) {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const sessionIdRef = useRef<string | null>(null);
  const sessionsByThreadRef = useRef<Map<string, TerminalSessionState>>(new Map());
  const sessionsByIdRef = useRef<Map<string, TerminalSessionState>>(new Map());
  const cleanupCheckInFlightRef = useRef<Set<string>>(new Set());
  const resizeFrameRef = useRef<number | null>(null);
  const pendingResizeRef = useRef<{ cols: number; rows: number } | null>(null);
  const [isSwitchingThread, setIsSwitchingThread] = useState(false);
  const [starting, setStarting] = useState(false);
  const [lastCommand, setLastCommand] = useState<string | null>(null);

  const threadKey = useMemo(() => {
    if (!thread) {
      return null;
    }
    return `${thread.providerId}:${thread.id}:${thread.projectPath}`;
  }, [thread]);

  const launchTarget = useMemo<SessionLaunchTarget | null>(() => {
    if (launchRequest) {
      return {
        mode: "new",
        key: `new:${launchRequest.launchId}`,
        launchId: launchRequest.launchId,
        providerId: launchRequest.providerId,
        projectPath: launchRequest.projectPath,
        knownThreadIds: launchRequest.knownThreadIds,
      };
    }
    if (!thread || !threadKey) {
      return null;
    }
    return {
      mode: "resume",
      key: threadKey,
      threadId: thread.id,
      providerId: thread.providerId,
      projectPath: thread.projectPath,
    };
  }, [launchRequest, thread, threadKey]);

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

  useEffect(() => {
    if (!hostRef.current) {
      return;
    }

    const terminal = new Terminal({
      allowProposedApi: true,
      cursorBlink: true,
      convertEol: true,
      fontSize: 12,
      lineHeight: 1.3,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
      theme: {
        background: "#0f1117",
        foreground: "#d6dbe4",
        cursor: "#8ab4f8",
        selectionBackground: "#334155",
      },
    });
    const fitAddon = new FitAddon();
    const unicode11Addon = new Unicode11Addon();

    terminal.loadAddon(fitAddon);
    terminal.loadAddon(unicode11Addon);
    terminal.unicode.activeVersion = "11";
    terminal.open(hostRef.current);
    tuneHelperTextarea();
    fitAddon.fit();
    terminal.writeln("AgentDock embedded terminal ready.");
    terminal.focus();
    const handleHostMouseDown = () => terminal.focus();
    hostRef.current.addEventListener("mousedown", handleHostMouseDown);

    const resizeObserver = new ResizeObserver(() => {
      fitAddon.fit();
      tuneHelperTextarea();
      queueRemoteResize(terminal.cols, terminal.rows);
    });
    resizeObserver.observe(hostRef.current);

    const helperTextarea = hostRef.current.querySelector(
      ".xterm-helper-textarea",
    ) as HTMLTextAreaElement | null;
    let skipNextInputEvent = false;
    const handleHelperBeforeInput = (event: Event) => {
      const inputEvent = event as InputEvent;
      if (inputEvent.inputType !== "insertFromDictation") {
        return;
      }
      const payload = inputEvent.data ?? "";
      if (payload) {
        writeInputToSession(payload);
      }
      skipNextInputEvent = true;
      inputEvent.preventDefault();
      inputEvent.stopImmediatePropagation();
    };
    const handleHelperInput = (event: Event) => {
      const inputEvent = event as InputEvent;
      if (inputEvent.inputType !== "insertFromDictation" && !skipNextInputEvent) {
        return;
      }
      const target = event.target as HTMLTextAreaElement | null;
      if (target?.value) {
        writeInputToSession(target.value);
        target.value = "";
      }
      skipNextInputEvent = false;
      inputEvent.preventDefault();
      inputEvent.stopImmediatePropagation();
    };
    const handleHelperPaste = (event: Event) => {
      const clipboardEvent = event as ClipboardEvent;
      const text = clipboardEvent.clipboardData?.getData("text/plain")?.trimEnd();
      if (!text) {
        return;
      }
      writeInputToSession(text);
      clipboardEvent.preventDefault();
      clipboardEvent.stopImmediatePropagation();
    };
    if (helperTextarea) {
      helperTextarea.addEventListener("beforeinput", handleHelperBeforeInput, true);
      helperTextarea.addEventListener("input", handleHelperInput, true);
      helperTextarea.addEventListener("paste", handleHelperPaste, true);
    }

    const dataSubscription = terminal.onData((data) => {
      writeInputToSession(data);
    });

    terminal.attachCustomKeyEventHandler((event: KeyboardEvent) => {
      if (event.isComposing) {
        return true;
      }

      const key = event.key.toLowerCase();
      const hasMod = event.metaKey || event.ctrlKey;

      // Claude Code friendly behavior: Shift+Enter inserts a newline.
      if (event.key === "Enter" && event.shiftKey) {
        writeInputToSession("\n");
        return false;
      }

      if (hasMod && key === "c" && terminal.hasSelection()) {
        const selected = terminal.getSelection();
        if (selected && navigator.clipboard?.writeText) {
          void navigator.clipboard.writeText(selected);
        }
        return false;
      }

      return true;
    });

    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;

    let unlistenOutput: (() => void) | null = null;
    let unlistenExit: (() => void) | null = null;
    let active = true;

    void (async () => {
      unlistenOutput = await listen<EmbeddedTerminalOutputPayload>(
        "embedded-terminal-output",
        (event) => {
          if (!active) {
            return;
          }

          const session = sessionsByIdRef.current.get(event.payload.sessionId);
          if (!session) {
            return;
          }

          appendSessionBuffer(session, event.payload.data);
          if (event.payload.sessionId === sessionIdRef.current) {
            terminal.write(event.payload.data);
          }
        },
      );

      unlistenExit = await listen<EmbeddedTerminalExitPayload>(
        "embedded-terminal-exit",
        (event) => {
          if (!active) {
            return;
          }

          const statusText =
            typeof event.payload.statusCode === "number"
              ? `${event.payload.statusCode}`
              : "unknown";
          const exitLine = `\r\n[process exited: ${statusText}]`;
          const session = sessionsByIdRef.current.get(event.payload.sessionId);
          if (!session) {
            return;
          }

          session.running = false;
          appendSessionBuffer(session, exitLine);
          sessionsByIdRef.current.delete(session.sessionId);
          const activeThreadSession = sessionsByThreadRef.current.get(session.threadKey);
          if (activeThreadSession?.sessionId === session.sessionId) {
            sessionsByThreadRef.current.delete(session.threadKey);
          }
          if (event.payload.sessionId === sessionIdRef.current) {
            terminal.write(exitLine);
            sessionIdRef.current = null;
          }
        },
      );
    })();

    return () => {
      active = false;
      dataSubscription.dispose();
      resizeObserver.disconnect();
      hostRef.current?.removeEventListener("mousedown", handleHostMouseDown);
      if (helperTextarea) {
        helperTextarea.removeEventListener("beforeinput", handleHelperBeforeInput, true);
        helperTextarea.removeEventListener("input", handleHelperInput, true);
        helperTextarea.removeEventListener("paste", handleHelperPaste, true);
      }
      if (unlistenOutput) {
        unlistenOutput();
      }
      if (unlistenExit) {
        unlistenExit();
      }
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
      if (resizeFrameRef.current !== null) {
        window.cancelAnimationFrame(resizeFrameRef.current);
        resizeFrameRef.current = null;
      }
      void closeAllSessions();
    };
  }, [
    appendSessionBuffer,
    closeAllSessions,
    queueRemoteResize,
    tuneHelperTextarea,
    writeInputToSession,
  ]);

  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal) {
      return;
    }

    let cancelled = false;

    const startSession = async () => {
      setIsSwitchingThread(true);
      setStarting(false);
      onError?.(null);
      setLastCommand(null);
      terminal.reset();
      terminal.clear();

      if (!launchTarget) {
        sessionIdRef.current = null;
        terminal.writeln("Select a thread from the left panel.");
        cleanupDormantSessions(null);
        setStarting(false);
        setIsSwitchingThread(false);
        return;
      }

      const existing = sessionsByThreadRef.current.get(launchTarget.key);
      if (existing) {
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

      setStarting(true);

      try {
        const response =
          launchTarget.mode === "resume"
            ? await invoke<StartEmbeddedTerminalResponse>("start_embedded_terminal", {
                request: {
                  threadId: launchTarget.threadId,
                  providerId: launchTarget.providerId,
                  projectPath: launchTarget.projectPath,
                  cols: Math.max(40, terminal.cols || 120),
                  rows: Math.max(12, terminal.rows || 36),
                },
              })
            : await invoke<StartEmbeddedTerminalResponse>("start_new_embedded_terminal", {
                request: {
                  providerId: launchTarget.providerId,
                  projectPath: launchTarget.projectPath,
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
        if (!cancelled) {
          setStarting(false);
          setIsSwitchingThread(false);
          fitAddonRef.current?.fit();
        }
      }
    };

    void startSession();

    return () => {
      cancelled = true;
    };
  }, [
    appendSessionBuffer,
    cleanupDormantSessions,
    launchTarget,
    onError,
    onLaunchRequestSettled,
    queueRemoteResize,
  ]);

  return (
    <div className="relative h-full w-full overflow-hidden bg-[#0f1117]">
      <div ref={hostRef} className="h-full w-full" />
      {isSwitchingThread ? (
        <div className="absolute inset-0 z-10 flex items-center justify-center bg-[#0f1117]/96">
          <div className="inline-flex items-center gap-2 rounded-md border border-slate-700/80 bg-slate-900/90 px-3 py-2 text-xs text-slate-200">
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
            Loading thread session...
          </div>
        </div>
      ) : null}
      <div className="pointer-events-none absolute left-3 top-2 text-[11px] text-slate-400/85">
        {starting ? (
          <span className="inline-flex items-center gap-1">
            <Loader2 className="h-3 w-3 animate-spin" />
            Starting terminal session...
          </span>
        ) : lastCommand ? (
          <span className="truncate">{lastCommand}</span>
        ) : null}
      </div>
      <div className="pointer-events-none absolute bottom-2 right-3 text-[10px] text-slate-400/75">
        Shift+Enter: newline · ⌘/Ctrl+C copy · ⌘/Ctrl+V paste
      </div>
    </div>
  );
}
