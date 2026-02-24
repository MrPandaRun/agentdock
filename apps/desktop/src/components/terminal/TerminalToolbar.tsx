import { CircleHelp, Ellipsis, Loader2, RefreshCw, Smartphone } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { RefObject } from "react";

import type { TerminalVisualTheme } from "@/components/terminal/theme";
import { Button } from "@/components/ui/button";

interface TerminalToolbarProps {
  helpOpen: boolean;
  helpButtonRef: RefObject<HTMLButtonElement | null>;
  isRefreshing: boolean;
  isOpeningHappy: boolean;
  isSwitchingThread: boolean;
  starting: boolean;
  theme: TerminalVisualTheme;
  onRefresh: () => void;
  onOpenHappy: () => void;
  onToggleHelp: () => void;
}

export function TerminalToolbar({
  helpOpen,
  helpButtonRef,
  isRefreshing,
  isOpeningHappy,
  isSwitchingThread,
  starting,
  theme,
  onRefresh,
  onOpenHappy,
  onToggleHelp,
}: TerminalToolbarProps) {
  const [moreOpen, setMoreOpen] = useState(false);
  const moreButtonRef = useRef<HTMLButtonElement | null>(null);
  const morePanelRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!moreOpen) {
      return;
    }

    const handleMouseDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target) {
        return;
      }
      if (morePanelRef.current?.contains(target)) {
        return;
      }
      if (moreButtonRef.current?.contains(target)) {
        return;
      }
      setMoreOpen(false);
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setMoreOpen(false);
      }
    };

    window.addEventListener("mousedown", handleMouseDown);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("mousedown", handleMouseDown);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [moreOpen]);

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
      <div className="relative">
        <Button
          ref={moreButtonRef}
          type="button"
          variant="outline"
          size="icon"
          className="h-7 w-7 border hover:opacity-90"
          style={{
            borderColor: theme.switchingChipBorder,
            backgroundColor: theme.switchingChipBackground,
            color: theme.switchingChipText,
          }}
          onClick={() => setMoreOpen((open) => !open)}
          aria-label="More terminal actions"
          aria-expanded={moreOpen}
          aria-haspopup="menu"
        >
          <Ellipsis className="h-3.5 w-3.5" />
        </Button>
        {moreOpen ? (
          <div
            ref={morePanelRef}
            className="absolute right-0 top-8 z-30 min-w-[150px] rounded-md border border-border bg-card p-1.5 shadow-md"
            role="menu"
          >
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-8 w-full justify-start gap-1.5 px-2 text-xs"
              onClick={() => {
                setMoreOpen(false);
                onOpenHappy();
              }}
              disabled={isOpeningHappy || isRefreshing || starting || isSwitchingThread}
              role="menuitem"
            >
              {isOpeningHappy ? (
                <Loader2 className="h-3 w-3 animate-spin" />
              ) : (
                <Smartphone className="h-3 w-3" />
              )}
              Open in Happy
            </Button>
          </div>
        ) : null}
      </div>
    </div>
  );
}
