import assert from "node:assert/strict";
import test from "node:test";
import {
  applyDirectoryTreeListing,
  beginDirectoryTreeRequest,
  createSftpDirectoryTreeState,
  directoryTreeAncestorPaths,
  ensureSftpDirectoryTreeState,
  failDirectoryTreeRequest,
  removeSftpDirectoryTreeState,
  selectDirectoryTreePath,
  visibleDirectoryTreeNodes,
} from "./directory-tree-state.ts";

const directory = (name, path, permissions = "drwxr-xr-x") => ({ name, path, permissions });

test("keeps directory trees isolated by SSH session", () => {
  const states = new Map();
  const left = ensureSftpDirectoryTreeState(states, "session-a");
  const right = ensureSftpDirectoryTreeState(states, "session-b");
  const version = beginDirectoryTreeRequest(left, "/");
  assert.equal(applyDirectoryTreeListing(left, "/", version, "/", [directory("home", "/home")]), true);
  assert.deepEqual(left.nodes.get("/").children, ["/home"]);
  assert.deepEqual(right.nodes.get("/").children, []);
});

test("ignores stale node responses", () => {
  const state = createSftpDirectoryTreeState("session-a");
  const stale = beginDirectoryTreeRequest(state, "/");
  const current = beginDirectoryTreeRequest(state, "/");
  assert.equal(applyDirectoryTreeListing(state, "/", stale, "/", [directory("old", "/old")]), false);
  assert.equal(applyDirectoryTreeListing(state, "/", current, "/", [directory("home", "/home")]), true);
  assert.deepEqual(state.nodes.get("/").children, ["/home"]);
});

test("refreshes child references while preserving loaded child state", () => {
  const state = createSftpDirectoryTreeState("session-a");
  let version = beginDirectoryTreeRequest(state, "/");
  applyDirectoryTreeListing(state, "/", version, "/", [directory("home", "/home"), directory("var", "/var")]);
  const home = state.nodes.get("/home");
  home.expanded = true;
  home.loaded = true;
  home.children = ["/home/test"];

  version = beginDirectoryTreeRequest(state, "/");
  applyDirectoryTreeListing(state, "/", version, "/", [directory("home", "/home"), directory("tmp", "/tmp")]);
  assert.deepEqual(state.nodes.get("/").children, ["/home", "/tmp"]);
  assert.equal(state.nodes.get("/home"), home);
  assert.equal(state.nodes.get("/home").expanded, true);
  assert.equal(state.nodes.get("/home").loaded, true);
});

test("selects canonical ancestor paths and expands parents", () => {
  const state = createSftpDirectoryTreeState("session-a");
  assert.deepEqual(directoryTreeAncestorPaths("/home/test/project"), ["/", "/home", "/home/test", "/home/test/project"]);
  const ancestors = selectDirectoryTreePath(state, "/home/test/project");
  assert.deepEqual(ancestors, ["/", "/home", "/home/test", "/home/test/project"]);
  assert.equal(state.selectedPath, "/home/test/project");
  assert.equal(state.nodes.get("/home/test").expanded, true);
  assert.equal(state.nodes.get("/home/test/project").expanded, false);
});

test("flattens only expanded nodes and reports node errors", () => {
  const state = createSftpDirectoryTreeState("session-a");
  const version = beginDirectoryTreeRequest(state, "/");
  applyDirectoryTreeListing(state, "/", version, "/", [directory("home", "/home"), directory("var", "/var")]);
  state.nodes.get("/home").expanded = true;
  state.nodes.get("/home").children = ["/home/test"];
  state.nodes.set("/home/test", {
    path: "/home/test",
    name: "test",
    parentPath: "/home",
    permissions: "drwxr-xr-x",
    children: [],
    loaded: false,
    loading: false,
    expanded: false,
    error: "",
    requestVersion: 0,
  });
  assert.deepEqual(visibleDirectoryTreeNodes(state).map(({ node, depth }) => [node.path, depth]), [
    ["/", 0],
    ["/home", 1],
    ["/home/test", 2],
    ["/var", 1],
  ]);

  const failedVersion = beginDirectoryTreeRequest(state, "/var");
  assert.equal(failDirectoryTreeRequest(state, "/var", failedVersion, "permission denied"), true);
  assert.equal(state.nodes.get("/var").error, "permission denied");
});

test("removes a session tree and invalidates its requests", () => {
  const states = new Map();
  const state = ensureSftpDirectoryTreeState(states, "session-a");
  const version = beginDirectoryTreeRequest(state, "/");
  removeSftpDirectoryTreeState(states, "session-a");
  assert.equal(states.has("session-a"), false);
  assert.notEqual(state.nodes.get("/").requestVersion, version);
});
