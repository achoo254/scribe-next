// Kiểu dùng chung meeting (caption envelope) — desktop lẫn web. Framework-agnostic.
export interface CaptionMsg {
  type: "caption";
  track: string;
  name: string;
  kind: "partial" | "final";
  text: string;
  t0: number;
  t1: number;
}

export type HostStatus = "ok" | "transcribe_down" | "mic_lost" | "overloaded";

export interface HostStatusMsg {
  type: "host_status";
  status: HostStatus;
}

export type MeetingDataMsg = CaptionMsg | HostStatusMsg;

export interface CaptionLine {
  id: string;
  track: string;
  name: string;
  text: string;
  final: boolean;
  t0: number;
  t1: number;
}

export interface TurnCredential {
  username: string;
  credential: string;
  ttl: number;
  urls: string[];
}

export interface TokenResult {
  token: string;
  url: string;
  identity: string;
  role: "host" | "guest";
  turn?: TurnCredential | null;
}
