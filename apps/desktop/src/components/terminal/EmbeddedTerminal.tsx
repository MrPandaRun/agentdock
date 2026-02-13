import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FitAddon } from "@xterm/addon-fit";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { Terminal } from "@xterm/xterm";
import { CircleHelp, Loader2, RefreshCw, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import "@xterm/xterm/css/xterm.css";

import { TERMINAL_THEMES } from "@/components/terminal/theme";
import { Button } from "@/components/ui/button";
import { isSupportedProvider, providerDisplayName } from "@/lib/provider";
import type { TerminalTheme, ThreadProviderId } from "@/types";

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
  terminalTheme: TerminalTheme;
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
const MODE_TOGGLE_SHORTCUT_LABEL = "Cmd/Ctrl+Shift+M";
const AGENT_MODE_SWITCH_SHORTCUT_LABEL = "Shift+Tab";

interface TerminalProviderHelpDoc {
  modeNote: string;
  quickStartSteps: string[];
  internalModesNote: string;
  internalModeSteps: string[];
  modelShortcutNote: string;
  troubleshootingSteps: string[];
  detailedDocsLabel: string;
  detailedDocsHref: string;
}

const TERMINAL_COMMON_SHORTCUTS = [
  "Enter: send current input",
  "Shift+Enter: insert newline without submitting",
  "Cmd/Ctrl+C: copy selected text",
  "Cmd/Ctrl+V: paste clipboard into the CLI session",
  `${MODE_TOGGLE_SHORTCUT_LABEL}: toggle UI/Terminal mode`,
  `${AGENT_MODE_SWITCH_SHORTCUT_LABEL}: switch agent mode/model in supported CLIs`,
] as const;

const TERMINAL_PROVIDER_HELP_DOCS: Record<ThreadProviderId, TerminalProviderHelpDoc> = {
  claude_code: {
    modeNote:
      "Runs Claude Code CLI directly in AgentDock Terminal and preserves native Claude session behavior.",
    quickStartSteps: [
      "Select a Claude thread on the left and keep the right pane in Terminal mode.",
      "Type your prompt and press Enter to submit. Use Shift+Enter for multiline prompts.",
      "If output stalls or context looks wrong, use Refresh to rebuild the embedded session.",
    ],
    internalModesNote:
      "Claude Code can expose internal working modes (for example planning-style flows) through slash commands, depending on CLI version.",
    internalModeSteps: [
      `Use ${AGENT_MODE_SWITCH_SHORTCUT_LABEL} to cycle mode/model options when the CLI shows a switcher.`,
      "Type `/` in terminal to open slash-command hints when needed.",
      "After switching mode, continue in the same thread so context is preserved.",
    ],
    modelShortcutNote:
      `In supported Claude Code builds, ${AGENT_MODE_SWITCH_SHORTCUT_LABEL} opens or cycles model/mode options. If not available, use slash commands such as /model.`,
    troubleshootingSteps: [
      "Session not connected: verify `claude` is installed and available in PATH.",
      "Model switch not responding: check whether your Claude Code version supports `/model`.",
      "Path errors: confirm the thread project path still exists and is accessible.",
    ],
    detailedDocsLabel: "Claude Code docs",
    detailedDocsHref: "https://docs.anthropic.com/en/docs/claude-code/overview",
  },
  codex: {
    modeNote:
      "Runs Codex CLI directly in AgentDock Terminal and resumes by session id for this thread.",
    quickStartSteps: [
      "Select a Codex thread, type in terminal, and press Enter to submit.",
      "Use Shift+Enter when preparing multiline instructions.",
      "If session resume behaves unexpectedly, click Refresh to relaunch with current provider/thread/project context.",
    ],
    internalModesNote:
      "Codex workflows may include internal agent modes such as planning/execution, depending on your CLI release and configuration.",
    internalModeSteps: [
      `Try ${AGENT_MODE_SWITCH_SHORTCUT_LABEL} first when a mode/model switch UI is available.`,
      "Use `/` to inspect available Codex slash commands.",
      "Confirm mode change in the prompt/status area before sending the next task.",
    ],
    modelShortcutNote:
      `Use ${AGENT_MODE_SWITCH_SHORTCUT_LABEL} for quick model/mode switching when supported. If unsupported in your Codex version, use the model-switch path in official docs.`,
    troubleshootingSteps: [
      "CLI unavailable: verify `codex` is installed and authenticated.",
      "Mode/model commands not found: check Codex CLI version and enabled features.",
      "Directory missing: verify project path was not moved or deleted.",
    ],
    detailedDocsLabel: "Codex docs",
    detailedDocsHref: "https://platform.openai.com/docs/codex",
  },
  opencode: {
    modeNote:
      "Runs OpenCode CLI directly in AgentDock Terminal and continues within the current session context.",
    quickStartSteps: [
      "Select an OpenCode thread first, then send instructions in terminal.",
      "Use Shift+Enter for multiline prompts before submitting.",
      "If session state looks inconsistent, use Refresh to reconnect with current context.",
    ],
    internalModesNote:
      "OpenCode may provide internal operation modes (including planning-oriented flows) through slash commands in supported versions.",
    internalModeSteps: [
      `Use ${AGENT_MODE_SWITCH_SHORTCUT_LABEL} if your OpenCode build provides a mode/model switcher.`,
      "Type `/` to list available OpenCode commands.",
      "Choose any mode-related command exposed by your CLI build.",
      "Keep working in the same thread to retain project and tool context.",
    ],
    modelShortcutNote:
      `Use ${AGENT_MODE_SWITCH_SHORTCUT_LABEL} for quick switching if available. Otherwise switch models via OpenCode documented commands.`,
    troubleshootingSteps: [
      "Session resume fails: verify `opencode` is in PATH and its data directory is reachable.",
      "Mode/model commands unavailable: verify OpenCode version and feature support.",
      "Thread path errors: confirm the thread project path is still valid.",
    ],
    detailedDocsLabel: "OpenCode docs",
    detailedDocsHref: "https://opencode.ai/docs",
  },
};

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
  terminalTheme,
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
  const helpButtonRef = useRef<HTMLButtonElement | null>(null);
  const helpPopoverRef = useRef<HTMLDivElement | null>(null);
  const lastHandledRefreshRequestRef = useRef(0);
  const [isSwitchingThread, setIsSwitchingThread] = useState(false);
  const [starting, setStarting] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [refreshRequestId, setRefreshRequestId] = useState(0);
  const [refreshError, setRefreshError] = useState<string | null>(null);
  const [helpOpen, setHelpOpen] = useState(false);
  const [lastCommand, setLastCommand] = useState<string | null>(null);
  const initialThemeRef = useRef(TERMINAL_THEMES[terminalTheme]);
  const activeTheme = TERMINAL_THEMES[terminalTheme];

  const threadKey = useMemo(() => {
    if (!thread) {
      return null;
    }
    return `${thread.providerId}:${thread.id}:${thread.projectPath}:${terminalTheme}`;
  }, [terminalTheme, thread]);

  const launchTarget = useMemo<SessionLaunchTarget | null>(() => {
    if (launchRequest) {
      return {
        mode: "new",
        key: `new:${launchRequest.launchId}:${terminalTheme}`,
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
  }, [launchRequest, terminalTheme, thread, threadKey]);

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

  useEffect(() => {
    if (!helpOpen) {
      return;
    }

    const handleMouseDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target) {
        return;
      }
      if (helpPopoverRef.current?.contains(target)) {
        return;
      }
      if (helpButtonRef.current?.contains(target)) {
        return;
      }
      setHelpOpen(false);
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setHelpOpen(false);
      }
    };

    window.addEventListener("mousedown", handleMouseDown);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handleMouseDown);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [helpOpen]);

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
      drawBoldTextInBrightColors: false,
      minimumContrastRatio: initialThemeRef.current.minimumContrastRatio,
      fontSize: 12,
      lineHeight: 1.0,
      fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
      theme: initialThemeRef.current.xterm,
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
    terminal.options.theme = activeTheme.xterm;
    terminal.options.minimumContrastRatio = activeTheme.minimumContrastRatio;

    const activeSessionId = sessionIdRef.current;
    if (!activeSessionId) {
      return;
    }
    const activeSession = sessionsByIdRef.current.get(activeSessionId);
    if (!activeSession) {
      return;
    }

    terminal.reset();
    terminal.clear();
    if (activeSession.buffer) {
      terminal.write(activeSession.buffer);
    }
  }, [activeTheme]);

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
    launchTarget,
    onError,
    onLaunchRequestSettled,
    queueRemoteResize,
    terminalTheme,
    refreshRequestId,
  ]);

  return (
    <div
      className="relative h-full w-full overflow-hidden"
      style={{ backgroundColor: activeTheme.containerBackground }}
    >
      <div className="h-full w-full px-3 pb-8 pt-8">
        <div ref={hostRef} className="h-full w-full" />
      </div>
      {isSwitchingThread ? (
        <div
          className="absolute inset-0 z-10 flex items-center justify-center"
          style={{ backgroundColor: activeTheme.switchingOverlayBackground }}
        >
          <div
            className="inline-flex items-center gap-2 rounded-md border px-3 py-2 text-xs"
            style={{
              backgroundColor: activeTheme.switchingChipBackground,
              borderColor: activeTheme.switchingChipBorder,
              color: activeTheme.switchingChipText,
            }}
          >
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
            Loading thread session...
          </div>
        </div>
      ) : null}
      <div
        className="pointer-events-none absolute left-3 top-2 text-[11px]"
        style={{ color: activeTheme.commandText }}
      >
        {isRefreshing ? (
          <span className="inline-flex items-center gap-1">
            <Loader2 className="h-3 w-3 animate-spin" />
            Refreshing terminal session...
          </span>
        ) : starting ? (
          <span className="inline-flex items-center gap-1">
            <Loader2 className="h-3 w-3 animate-spin" />
            Starting terminal session...
          </span>
        ) : lastCommand ? (
          <span className="truncate">{lastCommand}</span>
        ) : null}
      </div>
      {refreshError ? (
        <div
          className="pointer-events-none absolute left-3 top-7 max-w-[70%] rounded-md border px-2.5 py-1 text-[11px]"
          style={{
            borderColor: "rgba(244, 63, 94, 0.5)",
            backgroundColor:
              terminalTheme === "light"
                ? "rgba(255, 241, 242, 0.97)"
                : "rgba(127, 29, 29, 0.35)",
            color:
              terminalTheme === "light"
                ? "rgb(159, 18, 57)"
                : "rgba(254, 205, 211, 0.96)",
          }}
        >
          Refresh failed: {refreshError}
        </div>
      ) : null}
      <div className="absolute right-3 top-2 z-20 flex items-center gap-1.5">
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-7 gap-1.5 border px-2 text-[11px] hover:opacity-90"
          style={{
            borderColor: activeTheme.switchingChipBorder,
            backgroundColor: activeTheme.switchingChipBackground,
            color: activeTheme.switchingChipText,
          }}
          onClick={handleRefreshSession}
          disabled={isRefreshing || starting || isSwitchingThread}
        >
          {isRefreshing ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <RefreshCw className="h-3 w-3" />
          )}
          Refresh
        </Button>
        <Button
          ref={helpButtonRef}
          type="button"
          variant="outline"
          size="icon"
          className="h-7 w-7 border hover:opacity-90"
          style={{
            borderColor: activeTheme.switchingChipBorder,
            backgroundColor: activeTheme.switchingChipBackground,
            color: activeTheme.switchingChipText,
          }}
          onClick={() => setHelpOpen((open) => !open)}
          aria-label="Terminal help"
          aria-expanded={helpOpen}
          aria-haspopup="dialog"
        >
          <CircleHelp className="h-3.5 w-3.5" />
        </Button>
      </div>
      {helpOpen ? (
        <div
          ref={helpPopoverRef}
          role="dialog"
          aria-label="Terminal help"
          className="absolute right-3 top-11 z-20 w-[min(31rem,calc(100%-1.5rem))] rounded-lg border p-3 shadow-2xl"
          style={{
            borderColor: activeTheme.switchingChipBorder,
            backgroundColor: activeTheme.switchingChipBackground,
            color: activeTheme.switchingChipText,
          }}
        >
          <div className="mb-2 flex items-center justify-between">
            <p
              className="text-xs font-semibold uppercase tracking-[0.14em]"
              style={{ color: activeTheme.commandText }}
            >
              {activeProviderId
                ? `${providerDisplayName(activeProviderId)} Quick Guide`
                : "Terminal Quick Guide"}
            </p>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="h-6 w-6 hover:opacity-90"
              style={{ color: activeTheme.switchingChipText }}
              onClick={() => setHelpOpen(false)}
              aria-label="Close help"
            >
              <X className="h-3.5 w-3.5" />
            </Button>
          </div>
          {activeProviderHelpDoc ? (
            <div className="space-y-2 text-[11px] leading-relaxed">
              <section>
                <p className="font-semibold" style={{ color: activeTheme.commandText }}>
                  Mode overview
                </p>
                <p>{activeProviderHelpDoc.modeNote}</p>
              </section>
              <section>
                <p className="font-semibold" style={{ color: activeTheme.commandText }}>
                  Core workflow
                </p>
                <ul className="list-disc space-y-0.5 pl-4">
                  {activeProviderHelpDoc.quickStartSteps.map((step) => (
                    <li key={step}>{step}</li>
                  ))}
                </ul>
              </section>
              <section>
                <p className="font-semibold" style={{ color: activeTheme.commandText }}>
                  Shortcuts
                </p>
                <ul className="list-disc space-y-0.5 pl-4">
                  {TERMINAL_COMMON_SHORTCUTS.map((shortcut) => (
                    <li key={shortcut}>{shortcut}</li>
                  ))}
                </ul>
              </section>
              <section>
                <p className="font-semibold" style={{ color: activeTheme.commandText }}>
                  Agent internal modes
                </p>
                <p>{activeProviderHelpDoc.internalModesNote}</p>
                <ul className="list-disc space-y-0.5 pl-4">
                  {activeProviderHelpDoc.internalModeSteps.map((step) => (
                    <li key={step}>{step}</li>
                  ))}
                </ul>
              </section>
              <section>
                <p className="font-semibold" style={{ color: activeTheme.commandText }}>
                  Model selection
                </p>
                <p>{activeProviderHelpDoc.modelShortcutNote}</p>
              </section>
              <section>
                <p className="font-semibold" style={{ color: activeTheme.commandText }}>
                  Troubleshooting
                </p>
                <ul className="list-disc space-y-0.5 pl-4">
                  {activeProviderHelpDoc.troubleshootingSteps.map((step) => (
                    <li key={step}>{step}</li>
                  ))}
                </ul>
              </section>
              <section
                className="border-t pt-2"
                style={{ borderColor: activeTheme.switchingChipBorder }}
              >
                <p className="mb-1 font-semibold" style={{ color: activeTheme.commandText }}>
                  Detailed docs
                </p>
                <a
                  href={activeProviderHelpDoc.detailedDocsHref}
                  target="_blank"
                  rel="noreferrer"
                  className="inline-flex rounded-md border px-2 py-1 text-[10px] hover:opacity-90"
                  style={{
                    borderColor: activeTheme.switchingChipBorder,
                    color: activeTheme.switchingChipText,
                  }}
                >
                  {activeProviderHelpDoc.detailedDocsLabel}
                </a>
              </section>
            </div>
          ) : (
            <p
              className="text-[11px] leading-relaxed"
              style={{ color: activeTheme.hintText }}
            >
              Select a thread first to view the quick guide for the current provider.
            </p>
          )}
        </div>
      ) : null}
      <div
        className="pointer-events-none absolute bottom-2 right-3 max-w-[82%] text-right text-[10px]"
        style={{ color: activeTheme.hintText }}
      >
        Shift+Enter newline · Shift+Tab mode/model switch (provider-supported) ·
        ⌘/Ctrl+Shift+M toggle pane mode · ⌘/Ctrl+C copy · ⌘/Ctrl+V paste
      </div>
    </div>
  );
}
