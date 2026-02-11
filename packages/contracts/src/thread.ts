import type { ProviderId } from "./provider";

export type ThreadMessageRole = "system" | "user" | "assistant" | "tool";

export interface ThreadMessage {
  id: string;
  threadId: string;
  role: ThreadMessageRole;
  content: string;
  createdAt: string;
}

export interface ThreadSnapshot {
  threadId: string;
  providerId: ProviderId;
  projectPath: string;
  title: string;
  tags: string[];
  messages: ThreadMessage[];
}
