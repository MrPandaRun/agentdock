import { describe, expect, test } from "vitest";

import type { AgentThreadSummary } from "@/types";

import {
  pickCreatedThread,
  resolveSelectedThreadKey,
  threadKey,
} from "./thread";

function buildThread(input: Partial<AgentThreadSummary> & Pick<AgentThreadSummary, "id" | "providerId">): AgentThreadSummary {
  return {
    id: input.id,
    providerId: input.providerId,
    projectPath: input.projectPath ?? "/workspace/demo",
    title: input.title ?? "Demo",
    tags: input.tags ?? [],
    lastActiveAt: input.lastActiveAt ?? "1700000000000",
    lastMessagePreview: input.lastMessagePreview ?? null,
  };
}

describe("threadKey", () => {
  test("includes provider and id", () => {
    expect(
      threadKey({
        providerId: "claude_code",
        id: "thread-1",
      }),
    ).toBe("claude_code:thread-1");
  });
});

describe("resolveSelectedThreadKey", () => {
  test("keeps current selection when key still exists", () => {
    const threads = [
      buildThread({ providerId: "claude_code", id: "shared-id" }),
      buildThread({ providerId: "codex", id: "shared-id" }),
    ];
    const selected = resolveSelectedThreadKey(threads, "codex:shared-id");
    expect(selected).toBe("codex:shared-id");
  });

  test("falls back to first visible thread key", () => {
    const threads = [
      buildThread({ providerId: "opencode", id: "thread-a" }),
      buildThread({ providerId: "codex", id: "thread-b" }),
    ];
    const selected = resolveSelectedThreadKey(threads, "missing:key");
    expect(selected).toBe("opencode:thread-a");
  });
});

describe("pickCreatedThread", () => {
  test("does not treat same id from another provider as already known", () => {
    const threads = [
      buildThread({
        providerId: "claude_code",
        id: "same-id",
        projectPath: "/workspace/demo",
        lastActiveAt: "1700000000000",
      }),
      buildThread({
        providerId: "codex",
        id: "same-id",
        projectPath: "/workspace/demo",
        lastActiveAt: "1700000001000",
      }),
    ];

    const created = pickCreatedThread(threads, {
      providerId: "codex",
      projectPath: "/workspace/demo",
      knownThreadKeys: ["claude_code:same-id"],
    });

    expect(created).not.toBeNull();
    expect(created?.providerId).toBe("codex");
    expect(created?.id).toBe("same-id");
  });
});
