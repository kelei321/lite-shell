import { computed, ref } from "vue";
import {
  cancelSftpDirectoryBatch,
  createSftpDirectoryBatch,
  deleteSftpDirectoryBatch,
  enqueueSftpDirectoryBatch,
  listSftpDirectoryBatches,
  pauseSftpDirectoryBatch,
  resumeSftpDirectoryBatch,
  retrySftpDirectoryBatch,
  rollbackSftpDirectoryBatch,
  type ConflictStrategy,
  type DirectoryConflictStrategy,
  type SftpDirectoryBatch,
} from "../services/ssh";
import {
  isDirectoryBatchTerminal,
  mergeDirectoryBatchSnapshot,
  shouldApplyDirectoryBatch,
} from "./directory-batch-state";

type BatchWaiter = {
  resolve: (batch: SftpDirectoryBatch) => void;
  reject: (error: Error) => void;
};

export function useSftpDirectoryBatches() {
  const batches = ref<SftpDirectoryBatch[]>([]);
  const maxFilesPerBatch = ref(5_000);
  const waiters = new Map<string, Set<BatchWaiter>>();
  const visibleBatches = computed(() =>
    [...batches.value].sort((left, right) => right.createdAt - left.createdAt),
  );

  function terminalError(batch: SftpDirectoryBatch) {
    return new Error(
      batch.lastError
      ?? (batch.state === "rollback_required"
        ? "目录批次需要人工检查或回滚"
        : `目录批次已${batch.state === "cancelled" ? "取消" : "失败"}`),
    );
  }

  function settleWaiters(batch: SftpDirectoryBatch) {
    if (!isDirectoryBatchTerminal(batch.state)) return;
    const pending = waiters.get(batch.batchId);
    if (!pending) return;
    waiters.delete(batch.batchId);
    for (const waiter of pending) {
      if (batch.state === "completed") waiter.resolve(batch);
      else waiter.reject(terminalError(batch));
    }
  }

  function handleBatch(batch: SftpDirectoryBatch) {
    const index = batches.value.findIndex((item) => item.batchId === batch.batchId);
    const current = index >= 0 ? batches.value[index] : undefined;
    if (!shouldApplyDirectoryBatch(current, batch)) return;
    if (index >= 0) batches.value[index] = batch;
    else batches.value.push(batch);
    settleWaiters(batch);
  }

  async function refreshBatches() {
    const snapshot = await listSftpDirectoryBatches();
    maxFilesPerBatch.value = snapshot.maxFilesPerBatch;
    batches.value = mergeDirectoryBatchSnapshot(batches.value, snapshot);
    for (const batch of batches.value) settleWaiters(batch);
  }

  async function createBatch(request: {
    sessionId: string;
    serverLabel: string;
    direction: "upload" | "download";
    sourceDirectory: string;
    targetDirectory: string;
    conflictStrategy: DirectoryConflictStrategy;
    directories: string[];
    fileCount: number;
  }) {
    const batch = await createSftpDirectoryBatch(request);
    handleBatch(batch);
    return batch;
  }

  async function enqueueBatch(
    batchId: string,
    requests: Array<{
      localPath: string;
      remotePath: string;
      conflictStrategy: ConflictStrategy;
    }>,
  ) {
    const batch = await enqueueSftpDirectoryBatch(batchId, requests);
    handleBatch(batch);
    return batch;
  }

  function waitForBatch(batchId: string): Promise<SftpDirectoryBatch> {
    const current = batches.value.find((batch) => batch.batchId === batchId);
    if (current && isDirectoryBatchTerminal(current.state)) {
      return current.state === "completed"
        ? Promise.resolve(current)
        : Promise.reject(terminalError(current));
    }
    return new Promise((resolve, reject) => {
      const pending = waiters.get(batchId) ?? new Set<BatchWaiter>();
      pending.add({ resolve, reject });
      waiters.set(batchId, pending);
    });
  }

  async function runAction(
    batch: SftpDirectoryBatch,
    action: (batchId: string) => Promise<SftpDirectoryBatch>,
  ) {
    const updated = await action(batch.batchId);
    handleBatch(updated);
    return updated;
  }

  return {
    batches,
    visibleBatches,
    maxFilesPerBatch,
    handleBatch,
    refreshBatches,
    createBatch,
    enqueueBatch,
    waitForBatch,
    pauseBatch: (batch: SftpDirectoryBatch) => runAction(batch, pauseSftpDirectoryBatch),
    resumeBatch: (batch: SftpDirectoryBatch) => runAction(batch, resumeSftpDirectoryBatch),
    retryBatch: (batch: SftpDirectoryBatch) => runAction(batch, retrySftpDirectoryBatch),
    cancelBatch: async (batch: SftpDirectoryBatch, deletePartial: boolean) => {
      const updated = await cancelSftpDirectoryBatch(batch.batchId, deletePartial);
      handleBatch(updated);
      return updated;
    },
    rollbackBatch: (batch: SftpDirectoryBatch) => runAction(batch, rollbackSftpDirectoryBatch),
    deleteBatch: async (batch: SftpDirectoryBatch) => {
      await deleteSftpDirectoryBatch(batch.batchId);
      batches.value = batches.value.filter((item) => item.batchId !== batch.batchId);
    },
  };
}
