import { X } from "lucide-react";
import type { RefObject } from "react";

import type { TerminalProviderHelpDoc } from "@/components/terminal/helpDocs";
import { TERMINAL_COMMON_SHORTCUTS } from "@/components/terminal/helpDocs";
import type { TerminalVisualTheme } from "@/components/terminal/theme";
import { Button } from "@/components/ui/button";
import { providerDisplayName } from "@/lib/provider";
import type { ThreadProviderId } from "@/types";

interface TerminalHelpPopoverProps {
  open: boolean;
  popoverRef: RefObject<HTMLDivElement | null>;
  providerId: ThreadProviderId | null;
  providerHelpDoc: TerminalProviderHelpDoc | null;
  theme: TerminalVisualTheme;
  onClose: () => void;
}

export function TerminalHelpPopover({
  open,
  popoverRef,
  providerId,
  providerHelpDoc,
  theme,
  onClose,
}: TerminalHelpPopoverProps) {
  if (!open) {
    return null;
  }

  return (
    <div
      ref={popoverRef}
      role="dialog"
      aria-label="Terminal help"
      className="absolute right-3 top-11 z-20 w-[min(31rem,calc(100%-1.5rem))] rounded-lg border p-3 shadow-2xl"
      style={{
        borderColor: theme.switchingChipBorder,
        backgroundColor: theme.switchingChipBackground,
        color: theme.switchingChipText,
      }}
    >
      <div className="mb-2 flex items-center justify-between">
        <p
          className="text-xs font-semibold uppercase tracking-[0.14em]"
          style={{ color: theme.commandText }}
        >
          {providerId ? `${providerDisplayName(providerId)} Quick Guide` : "Terminal Quick Guide"}
        </p>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="h-6 w-6 hover:opacity-90"
          style={{ color: theme.switchingChipText }}
          onClick={onClose}
          aria-label="Close help"
        >
          <X className="h-3.5 w-3.5" />
        </Button>
      </div>

      {providerHelpDoc ? (
        <div className="space-y-2 text-[11px] leading-relaxed">
          <section>
            <p className="font-semibold" style={{ color: theme.commandText }}>
              Mode overview
            </p>
            <p>{providerHelpDoc.modeNote}</p>
          </section>
          <section>
            <p className="font-semibold" style={{ color: theme.commandText }}>
              Core workflow
            </p>
            <ul className="list-disc space-y-0.5 pl-4">
              {providerHelpDoc.quickStartSteps.map((step) => (
                <li key={step}>{step}</li>
              ))}
            </ul>
          </section>
          <section>
            <p className="font-semibold" style={{ color: theme.commandText }}>
              Shortcuts
            </p>
            <ul className="list-disc space-y-0.5 pl-4">
              {TERMINAL_COMMON_SHORTCUTS.map((shortcut) => (
                <li key={shortcut}>{shortcut}</li>
              ))}
            </ul>
          </section>
          <section>
            <p className="font-semibold" style={{ color: theme.commandText }}>
              Agent internal modes
            </p>
            <p>{providerHelpDoc.internalModesNote}</p>
            <ul className="list-disc space-y-0.5 pl-4">
              {providerHelpDoc.internalModeSteps.map((step) => (
                <li key={step}>{step}</li>
              ))}
            </ul>
          </section>
          <section>
            <p className="font-semibold" style={{ color: theme.commandText }}>
              Model selection
            </p>
            <p>{providerHelpDoc.modelShortcutNote}</p>
          </section>
          <section>
            <p className="font-semibold" style={{ color: theme.commandText }}>
              Troubleshooting
            </p>
            <ul className="list-disc space-y-0.5 pl-4">
              {providerHelpDoc.troubleshootingSteps.map((step) => (
                <li key={step}>{step}</li>
              ))}
            </ul>
          </section>
          <section className="border-t pt-2" style={{ borderColor: theme.switchingChipBorder }}>
            <p className="mb-1 font-semibold" style={{ color: theme.commandText }}>
              Detailed docs
            </p>
            <a
              href={providerHelpDoc.detailedDocsHref}
              target="_blank"
              rel="noreferrer"
              className="inline-flex rounded-md border px-2 py-1 text-[10px] hover:opacity-90"
              style={{
                borderColor: theme.switchingChipBorder,
                color: theme.switchingChipText,
              }}
            >
              {providerHelpDoc.detailedDocsLabel}
            </a>
          </section>
        </div>
      ) : (
        <p className="text-[11px] leading-relaxed" style={{ color: theme.hintText }}>
          Select a thread first to view the quick guide for the current provider.
        </p>
      )}
    </div>
  );
}
