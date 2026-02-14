export interface EmbeddedTerminalThread {
  id: string;
  providerId: string;
  projectPath: string;
}

export interface EmbeddedTerminalNewThreadLaunch {
  launchId: number;
  providerId: string;
  projectPath: string;
  knownThreadIds: string[];
}

export interface EmbeddedTerminalLaunchSettledPayload {
  launch: EmbeddedTerminalNewThreadLaunch;
  started: boolean;
}

export interface StartEmbeddedTerminalResponse {
  sessionId: string;
  command: string;
}

export interface EmbeddedTerminalOutputPayload {
  sessionId: string;
  data: string;
}

export interface EmbeddedTerminalExitPayload {
  sessionId: string;
  statusCode?: number;
}

export interface ThreadRuntimeState {
  agentAnswering: boolean;
  lastEventKind?: string | null;
  lastEventAtMs?: number | null;
}

export interface TerminalSessionState {
  threadKey: string;
  threadId: string | null;
  runtimeThreadId: string | null;
  providerId: string;
  sessionId: string;
  command: string;
  buffer: string;
  running: boolean;
  hasUserInput: boolean;
  lastTouchedAt: number;
}

export type SessionLaunchTarget =
  | {
      mode: "resume";
      key: string;
      threadId: string;
      providerId: string;
      projectPath: string;
    }
  | {
      mode: "new";
      key: string;
      launchId: number;
      providerId: string;
      projectPath: string;
      knownThreadIds: string[];
    };
