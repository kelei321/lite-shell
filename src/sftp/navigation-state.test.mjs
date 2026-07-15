import assert from "node:assert/strict";
import test from "node:test";
import {
  buildRemoteBreadcrumbs,
  isPointInsideRect,
  normalizeRemotePath,
  reconcileSelection,
  selectionsMatchSnapshot,
  updateSelectionPaths,
} from "./navigation-state.ts";

const entry = (path, sessionId = "session-a") => ({
  name: path.split("/").pop() || path,
  path,
  kind: "file",
  size: 1,
  permissions: "rw-r--r--",
  sessionId,
});

test("normalizes remote paths and builds clickable breadcrumbs", () => {
  assert.equal(normalizeRemotePath("/var//log/../tmp/"), "/var/tmp");
  assert.deepEqual(buildRemoteBreadcrumbs("/var/tmp"), [
    { label: "/", path: "/" },
    { label: "var", path: "/var" },
    { label: "tmp", path: "/var/tmp" },
  ]);
  assert.deepEqual(buildRemoteBreadcrumbs("."), [{ label: ".", path: "." }]);
});

test("reconciles a selection against refreshed entries", () => {
  const previous = [entry("/a.txt"), entry("/removed.txt")];
  const refreshed = [entry("/a.txt"), entry("/b.txt")];
  const result = reconcileSelection(refreshed, previous);
  assert.deepEqual(result.map((item) => item.path), ["/a.txt"]);
  assert.equal(result[0], refreshed[0]);
});

test("requires an exact selection snapshot before destructive actions", () => {
  assert.equal(selectionsMatchSnapshot(["/a", "/b"], ["/b", "/a"]), true);
  assert.equal(selectionsMatchSnapshot(["/a", "/b", "/c"], ["/a", "/b"]), false);
  assert.equal(selectionsMatchSnapshot(["/a"], ["/a", "/b"]), false);
});

test("supports contiguous shift ranges and additive ranges", () => {
  const paths = ["/a", "/b", "/c", "/d"];
  assert.deepEqual(
    updateSelectionPaths(paths, ["/b"], "/d", "/b", {
      toggle: false,
      range: true,
      additiveRange: false,
    }),
    { paths: ["/b", "/c", "/d"], anchorPath: "/b" },
  );
  assert.deepEqual(
    updateSelectionPaths(paths, ["/a"], "/d", "/c", {
      toggle: false,
      range: true,
      additiveRange: true,
    }),
    { paths: ["/a", "/c", "/d"], anchorPath: "/c" },
  );
});

test("supports ctrl or command toggling", () => {
  const paths = ["/a", "/b", "/c"];
  assert.deepEqual(
    updateSelectionPaths(paths, ["/a", "/b"], "/a", "/b", {
      toggle: true,
      range: false,
      additiveRange: false,
    }),
    { paths: ["/b"], anchorPath: "/a" },
  );
});

test("checks whether a physical drop point is inside the SFTP pane", () => {
  const rect = { left: 100, top: 50, right: 500, bottom: 400 };
  assert.equal(isPointInsideRect({ x: 100, y: 50 }, rect), true);
  assert.equal(isPointInsideRect({ x: 501, y: 50 }, rect), false);
  assert.equal(isPointInsideRect({ x: 200, y: 401 }, rect), false);
});
