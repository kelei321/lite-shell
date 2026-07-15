import assert from "node:assert/strict";
import test from "node:test";
import {
  mergeTransferQueueSnapshot,
  shouldApplyTransferTask,
} from "./transfer-queue-state.ts";

function task(taskId, state, updatedAt) {
  return {
    version: 1,
    taskId,
    serverId: "server-a",
    serverLabel: "Server A",
    direction: "upload",
    sourcePath: `C:\\${taskId}.txt`,
    targetPath: `/tmp/${taskId}.txt`,
    fileName: `${taskId}.txt`,
    conflictStrategy: "overwrite",
    state,
    transferred: state === "completed" ? 100 : 0,
    total: 100,
    speedBytesPerSecond: 0,
    resumedFrom: 0,
    checkpointAvailable: state !== "completed",
    allowPause: true,
    createdAt: 1,
    updatedAt,
  };
}

test("does not let an older enqueue response replace a completed event", () => {
  const completed = task("fast", "completed", 12);
  const queued = task("fast", "queued", 10);

  assert.equal(shouldApplyTransferTask(completed, queued), false);
  assert.equal(shouldApplyTransferTask(queued, completed), true);
});

test("keeps a task event that happened after a snapshot was generated", () => {
  const current = [task("late", "completed", 21)];
  const snapshot = { generatedAt: 20, concurrency: 3, tasks: [] };

  assert.deepEqual(mergeTransferQueueSnapshot(current, snapshot), current);
});

test("removes completed tasks omitted by a newer clear snapshot", () => {
  const current = [task("cleared", "completed", 20)];
  const snapshot = { generatedAt: 21, concurrency: 3, tasks: [] };

  assert.deepEqual(mergeTransferQueueSnapshot(current, snapshot), []);
});

test("prefers a newer event over an older copy inside the snapshot", () => {
  const running = task("copy", "running", 30);
  const queued = task("copy", "queued", 29);
  const snapshot = { generatedAt: 28, concurrency: 3, tasks: [queued] };

  assert.deepEqual(mergeTransferQueueSnapshot([running], snapshot), [running]);
});
