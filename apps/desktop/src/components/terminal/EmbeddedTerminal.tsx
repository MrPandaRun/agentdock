import { Loader2 } from "lucide-react";
import { useEffect, useState } from "react";

import "@xterm/xterm/css/xterm.css";

import {
  AGENT_MODE_SWITCH_SHORTCUT_LABEL,
} from "@/components/terminal/helpDocs";
import { TerminalHelpPopover } from "@/components/terminal/TerminalHelpPopover";
import { TerminalToolbar } from "@/components/terminal/TerminalToolbar";
import type {
  EmbeddedTerminalLaunchSettledPayload,
  EmbeddedTerminalNewThreadLaunch,
  EmbeddedTerminalThread,
} from "@/components/terminal/types";
import { useEmbeddedTerminalController } from "@/components/terminal/useEmbeddedTerminalController";
import type { TerminalTheme } from "@/types";

interface EmbeddedTerminalProps {
  thread: EmbeddedTerminalThread | null;
  terminalTheme: TerminalTheme;
  launchRequest?: EmbeddedTerminalNewThreadLaunch | null;
  onLaunchRequestSettled?: (payload: EmbeddedTerminalLaunchSettledPayload) => void;
  onActiveSessionExit?: () => void;
  onError?: (message: string | null) => void;
}

export function EmbeddedTerminal({
  thread,
  terminalTheme,
  launchRequest,
  onLaunchRequestSettled,
  onActiveSessionExit,
  onError,
}: EmbeddedTerminalProps) {
  const [helpOpen, setHelpOpen] = useState(false);
  const {
    hostRef,
    helpButtonRef,
    helpPopoverRef,
    activeTheme,
    activeProviderId,
    activeProviderHelpDoc,
    isSwitchingThread,
    starting,
    isRefreshing,
    isOpeningHappy,
    refreshError,
    happyError,
    lastCommand,
    handleRefreshSession,
    handleOpenInHappy,
  } = useEmbeddedTerminalController({
    thread,
    terminalTheme,
    launchRequest,
    onLaunchRequestSettled,
    onActiveSessionExit,
    onError,
  });

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
  }, [helpOpen, helpButtonRef, helpPopoverRef]);

  return (
    <div
      className="relative h-full w-full overflow-hidden"
      style={{ backgroundColor: activeTheme.containerBackground }}
    >
      <div className="h-full w-full px-1 pb-8 pt-8">
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
        className="pointer-events-none absolute left-1.5 top-2 text-[11px]"
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
          className="pointer-events-none absolute left-1.5 top-7 max-w-[70%] rounded-md border px-2.5 py-1 text-[11px]"
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
      ) : happyError ? (
        <div
          className="pointer-events-none absolute left-1.5 top-7 max-w-[70%] rounded-md border px-2.5 py-1 text-[11px]"
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
          Happy integration failed: {happyError}
        </div>
      ) : null}
      <TerminalToolbar
        helpOpen={helpOpen}
        helpButtonRef={helpButtonRef}
        isRefreshing={isRefreshing}
        isOpeningHappy={isOpeningHappy}
        isSwitchingThread={isSwitchingThread}
        starting={starting}
        theme={activeTheme}
        onRefresh={handleRefreshSession}
        onOpenHappy={() => void handleOpenInHappy()}
        onToggleHelp={() => setHelpOpen((open) => !open)}
      />
      <TerminalHelpPopover
        open={helpOpen}
        popoverRef={helpPopoverRef}
        providerId={activeProviderId}
        providerHelpDoc={activeProviderHelpDoc}
        theme={activeTheme}
        onClose={() => setHelpOpen(false)}
      />
      <div
        className="pointer-events-none absolute bottom-2 right-3 max-w-[82%] text-right text-[10px]"
        style={{ color: activeTheme.hintText }}
      >
        Shift+Enter newline · {AGENT_MODE_SWITCH_SHORTCUT_LABEL} mode/model switch
        (provider-supported) · ⌘/Ctrl+C copy · ⌘/Ctrl+V paste
      </div>
    </div>
  );
}
