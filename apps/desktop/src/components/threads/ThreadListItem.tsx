import { cn } from "@/lib/utils";

export interface ThreadListThreadItem {
  id: string;
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
  return (
    <li className="w-full">
      <button
        type="button"
        className={cn(
          "block w-full rounded-md px-2 py-1.5 text-left transition-colors",
          "hover:bg-accent/60",
          isActive && "bg-primary/10 text-foreground hover:bg-primary/10",
        )}
        onClick={() => onSelectThread(thread.id)}
      >
        <div className="w-full min-w-0 space-y-0.5">
          <p className="truncate text-[12px] leading-snug text-foreground/95">
            {getPreview(thread)}
          </p>
          <p className="text-[11px] text-muted-foreground/85">
            {formatLastActive(thread.lastActiveAt)}
          </p>
        </div>
      </button>
    </li>
  );
}
