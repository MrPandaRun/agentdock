import { Bot, Code2, SquareTerminal } from "lucide-react";

import { cn } from "@/lib/utils";

export interface ThreadListThreadItem {
  id: string;
  providerId: string;
  title: string;
  lastActiveAt: string;
  lastMessagePreview?: string | null;
}

interface ThreadListItemProps<T extends ThreadListThreadItem> {
  thread: T;
  isActive: boolean;
  onSelectThread: (threadId: string) => void;
  formatLastActive: (raw: string) => string;
  getPreview: (thread: T) => string;
}

export function ThreadListItem<T extends ThreadListThreadItem>({
  thread,
  isActive,
  onSelectThread,
  formatLastActive,
  getPreview,
}: ThreadListItemProps<T>) {
  const isCodex = thread.providerId === "codex";
  const isOpenCode = thread.providerId === "opencode";

  return (
    <li className="w-full min-w-0">
      <button
        type="button"
        className={cn(
          "block w-full min-w-0 overflow-hidden rounded-md px-2 py-1.5 text-left transition-colors",
          "hover:bg-accent/60",
          isActive && "bg-primary/10 text-foreground hover:bg-primary/10",
        )}
        onClick={() => onSelectThread(thread.id)}
      >
        <div className="w-full min-w-0 space-y-0.5">
          <div className="flex min-w-0 items-start gap-1.5">
            <span
              className={cn(
                "mt-[1px] inline-flex shrink-0 items-center justify-center rounded-sm",
                isCodex
                  ? "text-[hsl(var(--brand-codex))]"
                  : isOpenCode
                    ? "text-[hsl(var(--brand-opencode))]"
                    : "text-[hsl(var(--brand-claude))]",
              )}
              aria-hidden
            >
              {isCodex ? (
                <SquareTerminal className="h-3.5 w-3.5" />
              ) : isOpenCode ? (
                <Code2 className="h-3.5 w-3.5" />
              ) : (
                <Bot className="h-3.5 w-3.5" />
              )}
            </span>
            <p className="min-w-0 truncate text-[12px] leading-snug text-foreground/95">
              {getPreview(thread)}
            </p>
          </div>
          <p className="text-[11px] text-muted-foreground/85">
            {formatLastActive(thread.lastActiveAt)}
          </p>
        </div>
      </button>
    </li>
  );
}
