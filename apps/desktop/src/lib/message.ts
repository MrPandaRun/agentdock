import type { ToolMessageParts } from "@/types";

export function splitToolMessage(content: string): ToolMessageParts {
  const lines = content
    .split("\n")
    .map((line) => line.replace(/\r/g, "").trimEnd())
    .filter((line) => line.trim().length > 0);

  if (lines.length === 0) {
    return { headline: "" };
  }

  const firstLine = lines[0].trim();
  const restLines = lines.slice(1);
  const ioFromFirst = parseIoLine(firstLine);
  if (ioFromFirst && restLines.length === 0) {
    return {
      headline: ioFromFirst.label,
      ioLabel: ioFromFirst.label,
      ioBody: ioFromFirst.body,
    };
  }

  const firstDetailLine = restLines[0]?.trim();
  const ioFromDetail = firstDetailLine ? parseIoLine(firstDetailLine) : null;
  if (ioFromDetail) {
    const blockLines = [ioFromDetail.body, ...restLines.slice(1)].filter(
      (line) => line.trim().length > 0,
    );
    return {
      headline: firstLine,
      ioLabel: ioFromDetail.label,
      ioBody: blockLines.join("\n"),
    };
  }

  return {
    headline: firstLine,
    detail: restLines.join("\n"),
  };
}

export function parseIoLine(
  line: string,
): { label: "IN" | "OUT"; body: string } | null {
  if (line.startsWith("IN ")) {
    return { label: "IN", body: line.slice(3).trim() };
  }
  if (line === "IN") {
    return { label: "IN", body: "" };
  }
  if (line.startsWith("OUT ")) {
    return { label: "OUT", body: line.slice(4).trim() };
  }
  if (line === "OUT") {
    return { label: "OUT", body: "" };
  }
  return null;
}

export function normalizeCodeBody(raw: string): string {
  return raw
    .split("\n")
    .map((line) => line.replace(/\t/g, "  "))
    .join("\n")
    .trim();
}

export function parseToolTitle(raw: string): { strong: string; rest?: string } {
  const line = raw.trim();
  if (!line) {
    return { strong: "" };
  }
  const firstSpace = line.indexOf(" ");
  if (firstSpace === -1) {
    return { strong: line };
  }
  return {
    strong: line.slice(0, firstSpace),
    rest: line.slice(firstSpace + 1).trim(),
  };
}
