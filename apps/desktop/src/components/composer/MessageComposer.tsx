import { FormEvent, KeyboardEvent, useState } from "react";
import { ArrowUp } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Separator } from "@/components/ui/separator";
import { CardFooter } from "@/components/ui/card";
import { cn } from "@/lib/utils";
import type { AgentThreadSummary } from "@/types";

export interface MessageComposerProps {
  selectedThread: AgentThreadSummary | null;
  canUseUiComposer: boolean;
  sending: boolean;
  onSendMessage: (content: string) => Promise<void>;
}

export function MessageComposer({
  selectedThread,
  canUseUiComposer,
  sending,
  onSendMessage,
}: MessageComposerProps) {
  const [draft, setDraft] = useState("");

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!selectedThread || sending) {
      return;
    }

    const content = draft.trim();
    if (!content) {
      return;
    }

    setDraft("");
    await onSendMessage(content);
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      event.currentTarget.form?.requestSubmit();
    }
  };

  return (
    <CardFooter className="p-2 pt-1">
      <form onSubmit={handleSubmit} className="w-full">
        <div className="mx-auto w-full max-w-[980px] rounded-xl border border-primary/25 bg-card shadow-sm">
          <Textarea
            value={draft}
            placeholder={
              !selectedThread
                ? "Select a thread first"
                : "Type your message"
            }
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={handleKeyDown}
            disabled={!selectedThread || sending}
            rows={2}
            className={cn(
              "min-h-[64px] max-h-[180px] resize-none border-0 bg-transparent px-3 py-2.5 text-[13px] placeholder:text-muted-foreground/70 focus-visible:ring-0 focus-visible:ring-offset-0",
              "text-foreground",
            )}
          />
          <Separator />
          <div className="flex items-center justify-end bg-muted/35 px-2.5 py-2 text-muted-foreground">
            <Button
              type="submit"
              size="icon"
              disabled={!selectedThread || sending || !draft.trim() || !canUseUiComposer}
              className="h-8 w-8 rounded-lg border border-primary/40 bg-primary text-primary-foreground hover:bg-primary/90"
            >
              <ArrowUp className="h-3.5 w-3.5" />
            </Button>
          </div>
        </div>
      </form>
    </CardFooter>
  );
}
