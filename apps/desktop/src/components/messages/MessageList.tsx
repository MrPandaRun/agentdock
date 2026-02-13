import { useEffect, useRef } from "react";

import { ScrollArea } from "@/components/ui/scroll-area";
import type { AgentThreadMessage, AgentThreadSummary } from "@/types";
import { cn } from "@/lib/utils";
import { normalizeCodeBody, parseToolTitle, splitToolMessage } from "@/lib/message";

export interface MessageListProps {
  selectedThread: AgentThreadSummary | null;
  canUseUiComposer: boolean;
  loadingMessages: boolean;
  messages: AgentThreadMessage[];
  error: string | null;
}

export function MessageList({
  selectedThread,
  canUseUiComposer,
  loadingMessages,
  messages,
  error,
}: MessageListProps) {
  const scrollHostRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    const frame = window.requestAnimationFrame(() => {
      const viewport = scrollHostRef.current?.querySelector(
        "[data-radix-scroll-area-viewport]",
      );
      if (!(viewport instanceof HTMLElement)) {
        return;
      }
      viewport.scrollTop = viewport.scrollHeight;
    });

    return () => {
      window.cancelAnimationFrame(frame);
    };
  }, [selectedThread?.id, loadingMessages, messages.length, error]);

  return (
    <div ref={scrollHostRef} className="h-full">
      <ScrollArea className="h-full px-5 py-3">
        {error ? (
          <div className="mb-3 rounded-md border border-destructive/40 bg-destructive/10 px-2.5 py-1.5 text-xs text-destructive">
            {error}
          </div>
        ) : null}
        {selectedThread && !canUseUiComposer ? (
          <div className="mb-3 rounded-md border border-primary/30 bg-primary/5 px-2.5 py-1.5 text-xs text-muted-foreground">
            UI send is currently supported for Claude Code only. Use Terminal mode for Codex/OpenCode.
          </div>
        ) : null}

        {!selectedThread ? (
          <p className="text-sm text-muted-foreground">
            Choose a thread from the left panel.
          </p>
        ) : loadingMessages ? (
          <p className="text-sm text-muted-foreground">
            Loading conversation messages...
          </p>
        ) : messages.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            No visible messages in this thread.
          </p>
        ) : (
          <ul className="space-y-1.5">
            {messages.map((message, index) => {
              const isTool = message.kind === "tool";
              const isUser = message.role === "user";
              const isLast = index === messages.length - 1;
              const toolParts = isTool
                ? splitToolMessage(message.content)
                : { headline: "", detail: undefined };
              const toolTitle = isTool
                ? parseToolTitle(toolParts.headline)
                : { strong: "", rest: undefined };

              return (
                <li
                  key={`${message.role}-${message.kind}-${message.timestampMs ?? index}-${index}`}
                  className="relative pl-8"
                >
                  {!isLast ? (
                    <span className="absolute left-[8px] top-3.5 h-[calc(100%+0.42rem)] w-px bg-border/75" />
                  ) : null}
                  <span
                    className={cn(
                      "absolute left-[4px] top-2 h-2.5 w-2.5 rounded-full border border-card shadow-sm",
                      isTool
                        ? "bg-emerald-500"
                        : isUser
                          ? "bg-primary/70"
                          : "bg-slate-500/70",
                    )}
                  />

                  {isTool ? (
                    <article className="space-y-0.5 pb-0.5">
                      <p className="whitespace-pre-wrap break-words text-[14px] leading-relaxed text-foreground">
                        <span className="font-semibold">{toolTitle.strong}</span>
                        {toolTitle.rest ? (
                          <span className="text-muted-foreground">
                            {" "}
                            {toolTitle.rest}
                          </span>
                        ) : null}
                      </p>
                      {toolParts.detail ? (
                        <p className="whitespace-pre-wrap break-words text-[14px] leading-relaxed text-muted-foreground">
                          {toolParts.detail}
                        </p>
                      ) : null}
                      {toolParts.ioLabel ? (
                        <div className="mt-1 overflow-hidden rounded-md border border-border/80 bg-muted/35">
                          <div className="border-b border-border/80 px-2 py-1 text-[11px] font-semibold tracking-wide text-muted-foreground">
                            {toolParts.ioLabel}
                          </div>
                          <pre className="max-h-48 overflow-auto px-2 py-1.5 font-mono text-[12px] leading-relaxed text-foreground">
                            {normalizeCodeBody(toolParts.ioBody ?? "")}
                          </pre>
                        </div>
                      ) : null}
                    </article>
                  ) : (
                    <div className="space-y-0.5 pb-0.5">
                      {isUser ? (
                        <article className="inline-flex max-w-[84%] rounded-lg border border-border bg-secondary/70 px-2.5 py-1 shadow-sm">
                          <p className="whitespace-pre-wrap break-words text-[13px] leading-relaxed">
                            {message.content}
                          </p>
                        </article>
                      ) : (
                        <p className="whitespace-pre-wrap break-words text-[13px] leading-relaxed text-foreground">
                          {message.content}
                        </p>
                      )}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </ScrollArea>
    </div>
  );
}
