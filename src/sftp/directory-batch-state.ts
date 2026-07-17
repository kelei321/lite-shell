import type {
  DirectoryBatchSnapshot,
  DirectoryBatchState,
  SftpDirectoryBatch,
} from "../services/ssh";

const terminalStates = new Set<DirectoryBatchState>([
  "completed",
  "failed",
  "cancelled",
  "rollback_required",
]);

export function isDirectoryBatchTerminal(state: DirectoryBatchState) {
  return terminalStates.has(state);
}

export function shouldApplyDirectoryBatch(
  current: SftpDirectoryBatch | undefined,
  incoming: SftpDirectoryBatch,
) {
  if (!current) return true;
  if (incoming.updatedAt !== current.updatedAt) {
    return incoming.updatedAt > current.updatedAt;
  }
  return !isDirectoryBatchTerminal(current.state)
    || isDirectoryBatchTerminal(incoming.state);
}

export function mergeDirectoryBatchSnapshot(
  current: SftpDirectoryBatch[],
  snapshot: DirectoryBatchSnapshot,
) {
  const currentById = new Map(current.map((batch) => [batch.batchId, batch]));
  const snapshotIds = new Set(snapshot.batches.map((batch) => batch.batchId));
  const merged = snapshot.batches.map((batch) => {
    const existing = currentById.get(batch.batchId);
    return existing && !shouldApplyDirectoryBatch(existing, batch) ? existing : batch;
  });
  for (const batch of current) {
    if (!snapshotIds.has(batch.batchId) && batch.updatedAt > snapshot.generatedAt) {
      merged.push(batch);
    }
  }
  return merged;
}
