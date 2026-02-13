import type { ThreadProviderId } from "@/types";

export const MODE_TOGGLE_SHORTCUT_LABEL = "Cmd/Ctrl+Shift+M";
export const AGENT_MODE_SWITCH_SHORTCUT_LABEL = "Shift+Tab";

export interface TerminalProviderHelpDoc {
  modeNote: string;
  quickStartSteps: string[];
  internalModesNote: string;
  internalModeSteps: string[];
  modelShortcutNote: string;
  troubleshootingSteps: string[];
  detailedDocsLabel: string;
  detailedDocsHref: string;
}

export const TERMINAL_COMMON_SHORTCUTS = [
  "Enter: send current input",
  "Shift+Enter: insert newline without submitting",
  "Cmd/Ctrl+C: copy selected text",
  "Cmd/Ctrl+V: paste clipboard into the CLI session",
  `${MODE_TOGGLE_SHORTCUT_LABEL}: toggle UI/Terminal mode`,
  `${AGENT_MODE_SWITCH_SHORTCUT_LABEL}: switch agent mode/model in supported CLIs`,
] as const;

export const TERMINAL_PROVIDER_HELP_DOCS: Record<ThreadProviderId, TerminalProviderHelpDoc> = {
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
