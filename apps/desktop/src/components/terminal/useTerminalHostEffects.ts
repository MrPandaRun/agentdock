import { listen } from "@tauri-apps/api/event";
import { FitAddon } from "@xterm/addon-fit";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { Terminal } from "@xterm/xterm";
import { useEffect } from "react";
import type { MutableRefObject } from "react";

import type { TerminalVisualTheme } from "@/components/terminal/theme";

import type {
  EmbeddedTerminalExitPayload,
  EmbeddedTerminalOutputPayload,
  TerminalSessionState,
} from "./types";

interface UseTerminalHostEffectsProps {
  hostRef: MutableRefObject<HTMLDivElement | null>;
  terminalRef: MutableRefObject<Terminal | null>;
  fitAddonRef: MutableRefObject<FitAddon | null>;
  sessionIdRef: MutableRefObject<string | null>;
  sessionsByThreadRef: MutableRefObject<Map<string, TerminalSessionState>>;
  sessionsByIdRef: MutableRefObject<Map<string, TerminalSessionState>>;
  resizeFrameRef: MutableRefObject<number | null>;
  activeTheme: TerminalVisualTheme;
  initialThemeRef: MutableRefObject<TerminalVisualTheme>;
  appendSessionBuffer: (session: TerminalSessionState, chunk: string) => void;
  closeAllSessions: () => Promise<void>;
  queueRemoteResize: (cols: number, rows: number) => void;
  tuneHelperTextarea: () => void;
  writeInputToSession: (data: string) => void;
}

export function useTerminalHostEffects({
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
}: UseTerminalHostEffectsProps) {
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
    fitAddonRef,
    hostRef,
    initialThemeRef,
    queueRemoteResize,
    resizeFrameRef,
    sessionIdRef,
    sessionsByIdRef,
    sessionsByThreadRef,
    terminalRef,
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
  }, [activeTheme, terminalRef]);
}
