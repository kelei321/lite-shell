import { computed, ref } from "vue";
import {
  cancelQueuedSftpTransfer,
  clearCompletedSftpTransfers,
  enqueueSftpTransfer,
  listSftpTransferQueue,
  pauseSftpTransfer,
  resumeSftpTransfer,
  retrySftpTransfer,
  setSftpTransferConcurrency,
  type ConflictStrategy,
  type TransferQueueTask,
} from "../services/ssh";
import {
  mergeTransferQueueSnapshot,
  shouldApplyTransferTask,
} from "./transfer-queue-state";

type QueueRequest = {
  sessionId: string;
  serverLabel: string;
  direction: "upload" | "download";
  localPath: string;
  remotePath: string;
  conflictStrategy: ConflictStrategy;
  allowPause?: boolean;
};

type TaskWaiter = {
  resolve: (task: TransferQueueTask) => void;
  reject: (error: Error) => void;
};

const terminalStates = new Set<TransferQueueTask["state"]>([
  "completed",
  "failed",
  "cancelled",
]);

export function useSftpTransferQueue() {
  const transfers = ref<TransferQueueTask[]>([]);
  const transferConcurrency = ref(3);
  const waiters = new Map<string, Set<TaskWaiter>>();

  const visibleTransfers = computed(() =>
    [...transfers.value].sort((left, right) => right.createdAt - left.createdAt),
  );

  function findTask(taskId: string) {
    return transfers.value.find((task) => task.taskId === taskId);
  }

  function terminalError(task: TransferQueueTask) {
    return new Error(
      task.message
      ?? (task.state === "cancelled" ? "传输任务已取消" : "传输任务失败"),
    );
  }

  function settleWaiters(task: TransferQueueTask) {
    if (!terminalStates.has(task.state)) return;
    const pending = waiters.get(task.taskId);
    if (!pending) return;
    waiters.delete(task.taskId);
    for (const waiter of pending) {
      if (task.state === "completed") waiter.resolve(task);
      else waiter.reject(terminalError(task));
    }
  }

  function handleTransfer(task: TransferQueueTask) {
    const index = transfers.value.findIndex((item) => item.taskId === task.taskId);
    const current = index >= 0 ? transfers.value[index] : undefined;
    if (!shouldApplyTransferTask(current, task)) return;
    const next = current?.availableSessionId && !task.availableSessionId
      ? { ...task, availableSessionId: current.availableSessionId }
      : task;
    if (index >= 0) transfers.value[index] = next;
    else transfers.value.push(next);
    settleWaiters(next);
  }

  async function refreshTransferQueue() {
    const snapshot = await listSftpTransferQueue();
    transferConcurrency.value = snapshot.concurrency;
    transfers.value = mergeTransferQueueSnapshot(transfers.value, snapshot);
    for (const task of transfers.value) settleWaiters(task);
  }

  async function enqueueTransfer(request: QueueRequest) {
    const task = await enqueueSftpTransfer(request);
    handleTransfer(task);
    return task;
  }

  function waitForTransferTask(taskId: string): Promise<TransferQueueTask> {
    const existing = findTask(taskId);
    if (existing && terminalStates.has(existing.state)) {
      return existing.state === "completed"
        ? Promise.resolve(existing)
        : Promise.reject(terminalError(existing));
    }
    return new Promise((resolve, reject) => {
      const pending = waiters.get(taskId) ?? new Set<TaskWaiter>();
      pending.add({ resolve, reject });
      waiters.set(taskId, pending);
    });
  }

  async function waitForTransferTasks(taskIds: string[]) {
    const results = await Promise.allSettled(taskIds.map(waitForTransferTask));
    const failure = results.find(
      (result): result is PromiseRejectedResult => result.status === "rejected",
    );
    if (failure) throw failure.reason;
    return results.map((result) => (result as PromiseFulfilledResult<TransferQueueTask>).value);
  }

  async function pauseTransfer(task: TransferQueueTask) {
    await pauseSftpTransfer(task.taskId);
    await refreshTransferQueue();
  }

  async function resumeTransfer(task: TransferQueueTask) {
    await resumeSftpTransfer(task.taskId);
    await refreshTransferQueue();
  }

  async function retryTransfer(task: TransferQueueTask) {
    await retrySftpTransfer(task.taskId);
    await refreshTransferQueue();
  }

  async function cancelTransfer(task: TransferQueueTask, deletePartial: boolean) {
    await cancelQueuedSftpTransfer(task.taskId, deletePartial);
    await refreshTransferQueue();
  }

  async function clearFinishedTransfers() {
    await clearCompletedSftpTransfers();
    await refreshTransferQueue();
  }

  async function changeTransferConcurrency(value: number) {
    const concurrency = Math.min(5, Math.max(1, Math.round(value)));
    await setSftpTransferConcurrency(concurrency);
    await refreshTransferQueue();
  }

  return {
    transfers,
    transferConcurrency,
    visibleTransfers,
    handleTransfer,
    refreshTransferQueue,
    enqueueTransfer,
    waitForTransferTask,
    waitForTransferTasks,
    pauseTransfer,
    resumeTransfer,
    retryTransfer,
    cancelTransfer,
    clearFinishedTransfers,
    changeTransferConcurrency,
  };
}
