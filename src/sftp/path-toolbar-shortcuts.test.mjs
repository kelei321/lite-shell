import assert from "node:assert/strict";
import test from "node:test";
import {
  isSftpPathShortcut,
  shouldHandleSftpPathShortcut,
} from "./path-toolbar-shortcuts.ts";

test("recognizes the SFTP path editing shortcuts", () => {
  assert.equal(isSftpPathShortcut({ key: "l", ctrlKey: true }), true);
  assert.equal(isSftpPathShortcut({ key: "L", metaKey: true }), true);
  assert.equal(isSftpPathShortcut({ key: "F6" }), true);
});

test("does not consume unrelated or modified shortcuts", () => {
  assert.equal(isSftpPathShortcut({ key: "l" }), false);
  assert.equal(isSftpPathShortcut({ key: "l", ctrlKey: true, shiftKey: true }), false);
  assert.equal(isSftpPathShortcut({ key: "F6", altKey: true }), false);
});

test("preserves terminal and dialog keyboard behavior", () => {
  const shortcut = { key: "l", ctrlKey: true };
  assert.equal(shouldHandleSftpPathShortcut(shortcut, {
    insideTerminal: false,
    insideDialog: false,
  }), true);
  assert.equal(shouldHandleSftpPathShortcut(shortcut, {
    insideTerminal: true,
    insideDialog: false,
  }), false);
  assert.equal(shouldHandleSftpPathShortcut({ key: "F6" }, {
    insideTerminal: false,
    insideDialog: true,
  }), false);
});
