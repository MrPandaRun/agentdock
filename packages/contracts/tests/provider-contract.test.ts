import { describe, expect, test } from "vitest";
import {
  SUPPORTED_PROVIDERS,
  type ProviderId,
  type ProviderHealthCheckResult,
} from "../src/provider";

describe("provider contract", () => {
  test("includes V1 provider ids", () => {
    expect(SUPPORTED_PROVIDERS).toEqual(["codex", "claude_code"]);
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
});
