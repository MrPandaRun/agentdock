import { CircleHelp, Loader2, RefreshCw } from "lucide-react";
import type { RefObject } from "react";

import type { TerminalVisualTheme } from "@/components/terminal/theme";
import { Button } from "@/components/ui/button";

interface TerminalToolbarProps {
  helpOpen: boolean;
  helpButtonRef: RefObject<HTMLButtonElement | null>;
  isRefreshing: boolean;
  isSwitchingThread: boolean;
  starting: boolean;
  theme: TerminalVisualTheme;
  onRefresh: () => void;
  onToggleHelp: () => void;
}

export function TerminalToolbar({
  helpOpen,
  helpButtonRef,
  isRefreshing,
  isSwitchingThread,
  starting,
  theme,
  onRefresh,
  onToggleHelp,
}: TerminalToolbarProps) {
  return (
    <div className="absolute right-3 top-2 z-20 flex items-center gap-1.5">
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="h-7 gap-1.5 border px-2 text-[11px] hover:opacity-90"
        style={{
          borderColor: theme.switchingChipBorder,
          backgroundColor: theme.switchingChipBackground,
          color: theme.switchingChipText,
        }}
        onClick={onRefresh}
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
          borderColor: theme.switchingChipBorder,
          backgroundColor: theme.switchingChipBackground,
          color: theme.switchingChipText,
        }}
        onClick={onToggleHelp}
        aria-label="Terminal help"
        aria-expanded={helpOpen}
        aria-haspopup="dialog"
      >
        <CircleHelp className="h-3.5 w-3.5" />
      </Button>
    </div>
  );
}
