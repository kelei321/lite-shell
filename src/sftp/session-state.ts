import type { SftpEntry } from "../services/ssh";

export type SessionSftpEntry = SftpEntry & { sessionId: string };

export type SftpSessionState = {
  sessionId: string;
  path: string;
  entries: SessionSftpEntry[];
  loading: boolean;
  error: string;
  selectedEntries: SessionSftpEntry[];
  history: string[];
  historyIndex: number;
  bookmarks: string[];
  recentPaths: string[];
  requestVersion: number;
};

export type SftpSessionStateStore = Map<string, SftpSessionState>;

export function createSftpSessionState(sessionId: string): SftpSessionState {
  return {
    sessionId,
    path: ".",
    entries: [],
    loading: false,
    error: "",
    selectedEntries: [],
    history: [],
    historyIndex: -1,
    bookmarks: [],
    recentPaths: [],
    requestVersion: 0,
  };
}

export function ensureSftpSessionState(
  states: SftpSessionStateStore,
  sessionId: string,
): SftpSessionState {
  const existing = states.get(sessionId);
  if (existing) return existing;
  const created = createSftpSessionState(sessionId);
  states.set(sessionId, created);
  return created;
}

export function beginSftpDirectoryRequest(state: SftpSessionState): number {
  state.requestVersion += 1;
  state.loading = true;
  state.error = "";
  state.selectedEntries = [];
  return state.requestVersion;
}

export function isCurrentSftpDirectoryRequest(
  states: SftpSessionStateStore,
  state: SftpSessionState,
  requestVersion: number,
): boolean {
  return states.get(state.sessionId) === state && state.requestVersion === requestVersion;
}

export function finishSftpDirectoryRequest(
  states: SftpSessionStateStore,
  state: SftpSessionState,
  requestVersion: number,
): boolean {
  if (!isCurrentSftpDirectoryRequest(states, state, requestVersion)) return false;
  state.loading = false;
  return true;
}

export function bindSftpEntries(
  sessionId: string,
  entries: SftpEntry[],
): SessionSftpEntry[] {
  return entries.map((entry) => ({ ...entry, sessionId }));
}

export function selectionBelongsToSession(
  entries: SessionSftpEntry[],
  sessionId: string,
): boolean {
  return entries.length > 0 && entries.every((entry) => entry.sessionId === sessionId);
}

export function removeSftpSessionState(
  states: SftpSessionStateStore,
  sessionId: string,
): void {
  const state = states.get(sessionId);
  if (state) state.requestVersion += 1;
  states.delete(sessionId);
}
