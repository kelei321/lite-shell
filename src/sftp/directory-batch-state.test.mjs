import assert from "node:assert/strict";
import test from "node:test";
import {
  isDirectoryBatchTerminal,
  mergeDirectoryBatchSnapshot,
  shouldApplyDirectoryBatch,
} from "./directory-batch-state.ts";

function batch(batchId, serverId, state, updatedAt) {
  return {
    version: 1,
    batchId,
    name: batchId,
    direction: "upload",
    serverId,
    serverLabel: serverId,
    sourceDirectory: `C:\\${batchId}`,
    targetDirectory: `/${batchId}`,
    writeDirectory: `/${batchId}`,
    conflictStrategy: "merge",
    taskIds: [],
    fileCount: 1,
    completedCount: state === "completed" ? 1 : 0,
    failedCount: state === "failed" ? 1 : 0,
    cancelledCount: state === "cancelled" ? 1 : 0,
    requiresCommit: false,
    requiresRollback: state === "rollback_required",
    commitPhase: state === "completed" ? "completed" : "prepared",
    state,
    createdAt: 1,
    updatedAt,
  };
}

test("old running snapshot cannot replace a completed batch event", () => {
  const completed = batch("fast", "server-a", "completed", 12);
  const running = batch("fast", "server-a", "running", 10);
  assert.equal(shouldApplyDirectoryBatch(completed, running), false);
});

test("same timestamp cannot downgrade a terminal batch", () => {
  const completed = batch("fast", "server-a", "completed", 12);
  const running = batch("fast", "server-a", "running", 12);
  assert.equal(shouldApplyDirectoryBatch(completed, running), false);
});

test("snapshot merge keeps batches from different servers isolated", () => {
  const serverA = batch("a", "server-a", "running", 10);
  const serverB = batch("b", "server-b", "paused", 11);
  const snapshot = {
    generatedAt: 12,
    maxFilesPerBatch: 5000,
    batches: [serverA, serverB],
  };
  assert.deepEqual(mergeDirectoryBatchSnapshot([], snapshot), [serverA, serverB]);
});

test("restart-safe states and all waiter terminal states are recognized", () => {
  for (const state of ["completed", "failed", "cancelled", "rollback_required"]) {
    assert.equal(isDirectoryBatchTerminal(state), true);
  }
  assert.equal(isDirectoryBatchTerminal("paused"), false);
  assert.equal(isDirectoryBatchTerminal("committing"), false);
});
