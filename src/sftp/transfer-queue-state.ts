import type { TransferQueueSnapshot, TransferQueueTask } from "../services/ssh";

export function shouldApplyTransferTask(
  current: TransferQueueTask | undefined,
  incoming: TransferQueueTask,
) {
  return !current || incoming.updatedAt >= current.updatedAt;
}

export function mergeTransferQueueSnapshot(
  current: TransferQueueTask[],
  snapshot: TransferQueueSnapshot,
) {
  const currentById = new Map(current.map((task) => [task.taskId, task]));
  const snapshotIds = new Set(snapshot.tasks.map((task) => task.taskId));
  const merged = snapshot.tasks.map((task) => {
    const existing = currentById.get(task.taskId);
    return existing && existing.updatedAt > task.updatedAt ? existing : task;
  });

  for (const task of current) {
    if (!snapshotIds.has(task.taskId) && task.updatedAt > snapshot.generatedAt) {
      merged.push(task);
    }
  }
  return merged;
}
