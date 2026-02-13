import type { TerminalTheme } from "@/types";

interface TerminalVisualTheme {
  minimumContrastRatio: number;
  containerBackground: string;
  switchingOverlayBackground: string;
  switchingChipBackground: string;
  switchingChipBorder: string;
  switchingChipText: string;
  hintText: string;
  commandText: string;
  xterm: {
    background: string;
    foreground: string;
    cursor: string;
    cursorAccent: string;
    selectionBackground: string;
    black: string;
    red: string;
    green: string;
    yellow: string;
    blue: string;
    magenta: string;
    cyan: string;
    white: string;
    brightBlack: string;
    brightRed: string;
    brightGreen: string;
    brightYellow: string;
    brightBlue: string;
    brightMagenta: string;
    brightCyan: string;
    brightWhite: string;
  };
}

export const TERMINAL_THEMES: Record<TerminalTheme, TerminalVisualTheme> = {
  dark: {
    minimumContrastRatio: 1,
    containerBackground: "#0f1117",
    switchingOverlayBackground: "rgba(15, 17, 23, 0.96)",
    switchingChipBackground: "rgba(15, 23, 42, 0.9)",
    switchingChipBorder: "rgba(71, 85, 105, 0.8)",
    switchingChipText: "rgba(226, 232, 240, 0.95)",
    hintText: "rgba(148, 163, 184, 0.75)",
    commandText: "rgba(148, 163, 184, 0.85)",
    xterm: {
      background: "#0f1117",
      foreground: "#d6dbe4",
      cursor: "#8ab4f8",
      cursorAccent: "#0f1117",
      selectionBackground: "#334155",
      black: "#1f2937",
      red: "#f87171",
      green: "#4ade80",
      yellow: "#facc15",
      blue: "#60a5fa",
      magenta: "#c084fc",
      cyan: "#22d3ee",
      white: "#e2e8f0",
      brightBlack: "#475569",
      brightRed: "#fca5a5",
      brightGreen: "#86efac",
      brightYellow: "#fde047",
      brightBlue: "#93c5fd",
      brightMagenta: "#d8b4fe",
      brightCyan: "#67e8f9",
      brightWhite: "#f8fafc",
    },
  },
  light: {
    minimumContrastRatio: 4.5,
    containerBackground: "#ffffff",
    switchingOverlayBackground: "rgba(255, 255, 255, 0.95)",
    switchingChipBackground: "rgba(255, 255, 255, 0.96)",
    switchingChipBorder: "rgba(145, 145, 145, 0.5)",
    switchingChipText: "rgba(15, 23, 42, 0.92)",
    hintText: "rgba(31, 41, 55, 0.68)",
    commandText: "rgba(17, 24, 39, 0.88)",
    xterm: {
      background: "#ffffff",
      foreground: "#000000",
      cursor: "#919191",
      cursorAccent: "#ffffff",
      selectionBackground: "#e5ecf1",
      black: "#000000",
      red: "#b45648",
      green: "#6caa71",
      yellow: "#c4ac62",
      blue: "#5685a8",
      magenta: "#ad64be",
      cyan: "#69c6c9",
      white: "#c1c8cc",
      brightBlack: "#666666",
      brightRed: "#df6c5a",
      brightGreen: "#79be7e",
      brightYellow: "#e5c872",
      brightBlue: "#49a2e1",
      brightMagenta: "#d389e5",
      brightCyan: "#77e1e5",
      brightWhite: "#d8e1e7",
    },
  },
};
