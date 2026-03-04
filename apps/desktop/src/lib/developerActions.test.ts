import { describe, expect, test } from "vitest";

import type { OpenTargetStatus } from "@/types";
import {
  buildIdeContextEnv,
  normalizeDefaultOpenTarget,
  parseProjectOpenUsageMap,
  parseIdeContextByThread,
  resolveQuickOpenTargetId,
  resolveDefaultOpenTarget,
  sortTargetsByProjectUsage,
  updateProjectOpenUsage,
  serializeIdeContextByThread,
  serializeProjectOpenUsageMap,
  setThreadIdeContextEnabled,
} from "@/lib/developerActions";

function createTarget(input: Partial<OpenTargetStatus> & Pick<OpenTargetStatus, "id">): OpenTargetStatus {
  return {
    id: input.id,
    label: input.label ?? input.id,
    installed: input.installed ?? true,
    available: input.available ?? true,
    detail: input.detail ?? null,
    kind: input.kind ?? "ide",
  };
}

describe("normalizeDefaultOpenTarget", () => {
  test("falls back to vscode for unknown values", () => {
    expect(normalizeDefaultOpenTarget("unknown")).toBe("vscode");
    expect(normalizeDefaultOpenTarget(null)).toBe("vscode");
  });

  test("accepts supported target id", () => {
    expect(normalizeDefaultOpenTarget("warp")).toBe("warp");
  });
});

describe("resolveDefaultOpenTarget", () => {
  test("keeps preferred target when available", () => {
    const targets = [createTarget({ id: "vscode", available: true })];
    expect(resolveDefaultOpenTarget("vscode", targets)).toBe("vscode");
  });

  test("falls back to first available target", () => {
    const targets = [
      createTarget({ id: "vscode", available: false }),
      createTarget({ id: "cursor", available: true }),
    ];
    expect(resolveDefaultOpenTarget("vscode", targets)).toBe("cursor");
  });
});

describe("ide context map serialization", () => {
  test("parses true flags only", () => {
    const parsed = parseIdeContextByThread(
      JSON.stringify({ "codex:1": true, "codex:2": false, "codex:3": "yes" }),
    );
    expect(parsed).toEqual({ "codex:1": true });
  });

  test("serializes truthy entries only", () => {
    const serialized = serializeIdeContextByThread({
      "codex:1": true,
      "codex:2": false,
    });
    expect(serialized).toBe(JSON.stringify({ "codex:1": true }));
  });

  test("setThreadIdeContextEnabled toggles entries", () => {
    const current = { "codex:1": true };
    const enabled = setThreadIdeContextEnabled(current, "codex:2", true);
    expect(enabled).toEqual({ "codex:1": true, "codex:2": true });

    const disabled = setThreadIdeContextEnabled(enabled, "codex:1", false);
    expect(disabled).toEqual({ "codex:2": true });
  });
});

describe("buildIdeContextEnv", () => {
  test("returns undefined when disabled", () => {
    const env = buildIdeContextEnv({
      enabled: false,
      threadKey: "codex:1",
      providerId: "codex",
      projectPath: "/tmp",
    });
    expect(env).toBeUndefined();
  });

  test("returns expected env payload when enabled", () => {
    const env = buildIdeContextEnv({
      enabled: true,
      threadKey: "codex:1",
      providerId: "codex",
      projectPath: "/tmp/project",
      gitBranch: "main",
    });
    expect(env).toEqual({
      AGENTDOCK_IDE_CONTEXT_ENABLED: "1",
      AGENTDOCK_IDE_CONTEXT_THREAD_KEY: "codex:1",
      AGENTDOCK_IDE_CONTEXT_PROVIDER_ID: "codex",
      AGENTDOCK_IDE_CONTEXT_PROJECT_PATH: "/tmp/project",
      AGENTDOCK_IDE_CONTEXT_GIT_BRANCH: "main",
    });
  });
});

describe("project usage map", () => {
  test("parses and serializes usage map", () => {
    const parsed = parseProjectOpenUsageMap(
      JSON.stringify({
        "/workspace/demo": {
          cursor: 100,
          vscode: 90,
          unknown: 99,
        },
      }),
    );
    expect(parsed).toEqual({
      "/workspace/demo": {
        cursor: 100,
        vscode: 90,
      },
    });

    expect(serializeProjectOpenUsageMap(parsed)).toBe(
      JSON.stringify({
        "/workspace/demo": {
          cursor: 100,
          vscode: 90,
        },
      }),
    );
  });

  test("updateProjectOpenUsage writes timestamp for project target", () => {
    const updated = updateProjectOpenUsage({}, "/workspace/demo", "cursor", 123);
    expect(updated).toEqual({
      "/workspace/demo": {
        cursor: 123,
      },
    });
  });
});

describe("target ordering and quick open", () => {
  test("sortTargetsByProjectUsage sorts by last used timestamp desc", () => {
    const targets = [
      createTarget({ id: "vscode" }),
      createTarget({ id: "cursor" }),
      createTarget({ id: "warp", kind: "terminal" }),
    ];

    const sorted = sortTargetsByProjectUsage(targets, {
      warp: 50,
      cursor: 100,
    });
    expect(sorted.map((target) => target.id)).toEqual(["cursor", "warp", "vscode"]);
  });

  test("sortTargetsByProjectUsage keeps not-installed targets at the end and promotes used targets", () => {
    const targets = [
      createTarget({ id: "vscode", available: true, installed: true }),
      createTarget({ id: "cursor", available: false, installed: true }),
      createTarget({ id: "windsurf", available: false, installed: false }),
      createTarget({ id: "warp", kind: "terminal", available: true, installed: true }),
    ];

    const sorted = sortTargetsByProjectUsage(targets, {
      cursor: 300,
      windsurf: 999,
      warp: 200,
      vscode: 100,
    });
    expect(sorted.map((target) => target.id)).toEqual([
      "cursor",
      "warp",
      "vscode",
      "windsurf",
    ]);
  });

  test("sortTargetsByProjectUsage falls back to registry order when usage is empty", () => {
    const targets = [
      createTarget({ id: "warp", kind: "terminal" }),
      createTarget({ id: "cursor" }),
      createTarget({ id: "vscode" }),
    ];

    const sorted = sortTargetsByProjectUsage(targets, undefined);
    expect(sorted.map((target) => target.id)).toEqual(["vscode", "cursor", "warp"]);
  });

  test("resolveQuickOpenTargetId prefers most recent available target", () => {
    const targets = [
      createTarget({ id: "vscode", available: true }),
      createTarget({ id: "cursor", available: true }),
      createTarget({ id: "windsurf", available: false }),
    ];

    expect(
      resolveQuickOpenTargetId(
        targets,
        {
          windsurf: 200,
          cursor: 100,
        },
        "vscode",
      ),
    ).toBe("cursor");
  });
});
