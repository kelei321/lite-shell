import assert from "node:assert/strict";
import test from "node:test";
import {
  beginSftpDirectoryRequest,
  bindSftpEntries,
  createSftpSessionState,
  ensureSftpSessionState,
  finishSftpDirectoryRequest,
  isCurrentSftpDirectoryRequest,
  removeSftpSessionState,
  selectionBelongsToSession,
} from "./session-state.ts";

test("keeps directory state isolated by session", () => {
  const states = new Map();
  const first = ensureSftpSessionState(states, "session-a");
  const second = ensureSftpSessionState(states, "session-b");

  first.path = "/srv/a";
  second.path = "/srv/b";
  first.entries = bindSftpEntries("session-a", [{
    name: "a.txt",
    path: "/srv/a/a.txt",
    kind: "file",
    size: 1,
    permissions: "-rw-r--r--",
  }]);

  assert.equal(first.path, "/srv/a");
  assert.equal(second.path, "/srv/b");
  assert.equal(second.entries.length, 0);
});

test("ignores an older response from the same session", () => {
  const states = new Map();
  const state = ensureSftpSessionState(states, "session-a");
  const firstRequest = beginSftpDirectoryRequest(state);
  const secondRequest = beginSftpDirectoryRequest(state);

  assert.equal(isCurrentSftpDirectoryRequest(states, state, firstRequest), false);
  assert.equal(isCurrentSftpDirectoryRequest(states, state, secondRequest), true);
  assert.equal(finishSftpDirectoryRequest(states, state, firstRequest), false);
  assert.equal(state.loading, true);
  assert.equal(finishSftpDirectoryRequest(states, state, secondRequest), true);
  assert.equal(state.loading, false);
});

test("ignores a late response after the session is removed", () => {
  const states = new Map();
  const state = ensureSftpSessionState(states, "session-a");
  const request = beginSftpDirectoryRequest(state);

  removeSftpSessionState(states, "session-a");

  assert.equal(isCurrentSftpDirectoryRequest(states, state, request), false);
  assert.equal(finishSftpDirectoryRequest(states, state, request), false);
});

test("binds selections to their owning session", () => {
  const entries = bindSftpEntries("session-a", [{
    name: "a.txt",
    path: "/a.txt",
    kind: "file",
    size: 1,
    permissions: "-rw-r--r--",
  }]);

  assert.equal(selectionBelongsToSession(entries, "session-a"), true);
  assert.equal(selectionBelongsToSession(entries, "session-b"), false);
  assert.equal(selectionBelongsToSession([], "session-a"), false);
});

test("creates a complete default state", () => {
  const state = createSftpSessionState("session-a");
  assert.deepEqual(
    {
      sessionId: state.sessionId,
      path: state.path,
      loading: state.loading,
      historyIndex: state.historyIndex,
      requestVersion: state.requestVersion,
    },
    {
      sessionId: "session-a",
      path: ".",
      loading: false,
      historyIndex: -1,
      requestVersion: 0,
    },
  );
});
