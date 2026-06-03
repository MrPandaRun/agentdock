import { describe, expect, test } from "vitest";
import {
  SUPPORTED_PROVIDERS,
  type ProviderId,
  type ProviderHealthCheckResult,
} from "../src/provider";
import type { DockAgentTask } from "../src/dock-agent";

describe("provider contract", () => {
  test("includes V1 provider ids", () => {
    expect(SUPPORTED_PROVIDERS).toEqual([
      "codex",
      "claude_code",
      "opencode",
    ]);
  });

  test("health check payload shape is stable", () => {
    const payload: ProviderHealthCheckResult = {
      providerId: "codex" as ProviderId,
      status: "healthy",
      checkedAt: "2026-02-11T00:00:00Z",
    };

    expect(payload.providerId).toBe("codex");
    expect(payload.status).toBe("healthy");
  });

  test("dock agent task payload can target a V1 provider thread", () => {
    const task: DockAgentTask = {
      id: "task-1",
      kind: "orchestration",
      status: "queued",
      title: "Review stale work",
      objective: "Inspect open worker threads and resume the highest-priority one.",
      providerId: "codex",
      targetThreadId: "thread-1",
      createdAt: "2026-06-03T00:00:00Z",
      updatedAt: "2026-06-03T00:00:00Z",
      metadataJson: "{}",
    };

    expect(task.providerId).toBe("codex");
    expect(task.status).toBe("queued");
  });
});
