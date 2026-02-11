import { Folder } from "lucide-react";

import { cn } from "@/lib/utils";

import { ThreadListItem, type ThreadListThreadItem } from "./ThreadListItem";

export interface ThreadFolderGroupItem<T extends ThreadListThreadItem = ThreadListThreadItem> {
  key: string;
  folderName: string;
  threads: T[];
}

interface ThreadFolderGroupProps<T extends ThreadListThreadItem> {
  group: ThreadFolderGroupItem<T>;
  isActiveFolder: boolean;
  selectedThreadId: string | null;
  onSelectThread: (threadId: string) => void;
  formatLastActive: (raw: string) => string;
  getPreview: (thread: T) => string;
}

export function ThreadFolderGroup<T extends ThreadListThreadItem>({
  group,
  isActiveFolder,
  selectedThreadId,
  onSelectThread,
  formatLastActive,
  getPreview,
}: ThreadFolderGroupProps<T>) {
  return (
    <li className="w-full">
      <div className="w-full space-y-1">
        <div
          className={cn(
            "flex w-full items-center justify-between px-2 text-[11px] font-medium text-muted-foreground",
            isActiveFolder && "text-foreground",
          )}
        >
          <span className="inline-flex min-w-0 items-center gap-1.5">
            <Folder className="h-3.5 w-3.5 shrink-0" />
            <span className="truncate">{group.folderName}</span>
          </span>
          <span className="shrink-0 text-[10px]">{group.threads.length}</span>
        </div>

        <ul className="box-border w-full space-y-0.5 pl-3">
          {group.threads.map((thread) => (
            <ThreadListItem
              key={thread.id}
              thread={thread}
              isActive={thread.id === selectedThreadId}
              onSelectThread={onSelectThread}
              formatLastActive={formatLastActive}
              getPreview={getPreview}
            />
          ))}
        </ul>
      </div>
    </li>
  );
}
