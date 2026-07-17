import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type PasswordAuth = { type: "password"; password: string };
export type PrivateKeyAuth = { type: "private_key"; path: string; passphrase?: string };
export type AuthMethod = PasswordAuth | PrivateKeyAuth;

export type ConnectRequest = {
  sessionId: string;
  host: string;
  port: number;
  username: string;
  auth: AuthMethod;
  cols: number;
  rows: number;
  expectedHostFingerprint?: string;
};

export type ConnectOutcome =
  | {
      status: "connected";
      sessionId: string;
      host: string;
      port: number;
      username: string;
    }
  | {
      status: "host_key_confirmation_required";
      fingerprint: string;
      algorithm: string;
    };

export type SshEvent = {
  sessionId: string;
  kind: "connecting" | "connected" | "data" | "error" | "exit" | "disconnected";
  dataBase64?: string;
  message?: string;
  exitStatus?: number;
};

export type CommandError = {
  code: string;
  message: string;
  fingerprint?: string;
};

export type ProfileAuthType = "password" | "private_key";

export type ConnectionProfile = {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  authType: ProfileAuthType;
  privateKeyPath?: string;
  group: string;
  folderId: string;
  sortOrder: number;
  favorite: boolean;
  hasSecret: boolean;
  updatedAt: number;
};

export type SaveProfileRequest = {
  id?: string;
  name: string;
  host: string;
  port: number;
  username: string;
  authType: ProfileAuthType;
  privateKeyPath?: string;
  group?: string;
  folderId?: string;
  favorite?: boolean;
  secret?: string;
  rememberSecret: boolean;
};

export type ConnectionFolder = {
  id: string;
  name: string;
  parentId: string | null;
  sortOrder: number;
};

export type ConnectionManagerSnapshot = {
  version: number;
  folders: ConnectionFolder[];
  profiles: ConnectionProfile[];
};

export type ImportSource = "lite_shell" | "open_ssh" | "final_shell";

export type ImportPreview = {
  source: ImportSource;
  path: string;
  newCount: number;
  duplicateCount: number;
  skippedCount: number;
  folders: string[];
  warnings: string[];
};

export type ImportResult = {
  imported: number;
  duplicates: number;
  skipped: number;
  warnings: string[];
};

export type SftpEntry = {
  name: string;
  path: string;
  kind: "directory" | "file" | "symlink" | "other";
  size: number;
  modifiedAt?: number;
  permissions: string;
};

export type DirectoryListing = {
  path: string;
  entries: SftpEntry[];
};

export type SftpDirectoryTreeEntry = {
  name: string;
  path: string;
  permissions: string;
};

export type DirectoryTreeListing = {
  path: string;
  directories: SftpDirectoryTreeEntry[];
};

export type RecursiveScanSummary = {
  fileCount: number;
  directoryCount: number;
  totalSize: number;
  skippedLinks: number;
  skippedUnsupported: number;
  warnings: string[];
};

export type LocalDirectoryManifest = RecursiveScanSummary & {
  rootName: string;
  directories: string[];
  files: Array<{ absolutePath: string; relativePath: string; size: number }>;
};

export type RemoteDirectoryManifest = RecursiveScanSummary & {
  rootPath: string;
  directories: string[];
  files: Array<{ remotePath: string; relativePath: string; size: number }>;
};

export type ConflictStrategy = "overwrite" | "skip" | "rename";
export type TransferQueueState =
  | "queued"
  | "running"
  | "pausing"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";

export type TransferQueueTask = {
  version: number;
  taskId: string;
  batchId?: string;
  attemptId?: string;
  sessionId?: string;
  availableSessionId?: string;
  serverId: string;
  serverLabel: string;
  direction: "upload" | "download";
  sourcePath: string;
  targetPath: string;
  fileName: string;
  conflictStrategy: ConflictStrategy;
  state: TransferQueueState;
  transferred: number;
  total: number;
  speedBytesPerSecond: number;
  etaSeconds?: number | null;
  resumedFrom: number;
  message?: string;
  checkpointAvailable: boolean;
  allowPause: boolean;
  createdAt: number;
  updatedAt: number;
};

export type TransferQueueSnapshot = {
  generatedAt: number;
  concurrency: number;
  tasks: TransferQueueTask[];
};

export type DirectoryConflictStrategy = "merge" | "skip" | "rename" | "replace";
export type LocalPathKind = "missing" | "file" | "directory" | "symlink" | "other";
export type LocalPathInspection = { kind: LocalPathKind; size?: number };

export type DirectoryBatchState =
  | "preparing"
  | "queued"
  | "running"
  | "paused"
  | "committing"
  | "completed"
  | "failed"
  | "cancelled"
  | "rollback_required";

export type DirectoryCommitPhase =
  | "prepared"
  | "committing"
  | "cleanup_pending"
  | "completed"
  | "rollback_pending"
  | "rolled_back";

export type SftpDirectoryBatch = {
  version: number;
  batchId: string;
  name: string;
  direction: "upload" | "download";
  serverId: string;
  sessionId?: string;
  serverLabel: string;
  sourceDirectory: string;
  targetDirectory: string;
  writeDirectory: string;
  conflictStrategy: DirectoryConflictStrategy;
  replacementId?: string;
  stagingPath?: string;
  backupPath?: string;
  taskIds: string[];
  fileCount: number;
  completedCount: number;
  failedCount: number;
  cancelledCount: number;
  requiresCommit: boolean;
  requiresRollback: boolean;
  commitPhase: DirectoryCommitPhase;
  state: DirectoryBatchState;
  createdAt: number;
  updatedAt: number;
  lastError?: string;
};

export type DirectoryBatchSnapshot = {
  generatedAt: number;
  maxFilesPerBatch: number;
  batches: SftpDirectoryBatch[];
};

export type DiskMetrics = {
  path: string;
  total: number;
  used: number;
  usagePercent: number;
};

export type SystemMetrics = {
  uptimeSeconds: number;
  cpuUsagePercent: number;
  cpuCores: number;
  loadAverage: [number, number, number];
  memoryTotal: number;
  memoryUsed: number;
  swapTotal: number;
  swapUsed: number;
  networkRxBytesPerSecond: number;
  networkTxBytesPerSecond: number;
  latencyMs: number;
  disks: DiskMetrics[];
};

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export const isTauri = () => Boolean(window.__TAURI_INTERNALS__);

export const connectSsh = (request: ConnectRequest) =>
  invoke<ConnectOutcome>("ssh_connect", { request });

export const connectProfile = (request: {
  profileId: string;
  sessionId: string;
  cols: number;
  rows: number;
  expectedHostFingerprint?: string;
}) => invoke<ConnectOutcome>("ssh_connect_profile", { request });

export const listProfiles = () => invoke<ConnectionProfile[]>("profiles_list");

export const saveProfile = (request: SaveProfileRequest) =>
  invoke<ConnectionProfile>("profile_save", { request });

export const deleteProfile = (profileId: string) =>
  invoke<void>("profile_delete", { profileId });

export const getConnectionManagerSnapshot = () =>
  invoke<ConnectionManagerSnapshot>("connection_manager_snapshot");

export const duplicateProfile = (profileId: string) =>
  invoke<ConnectionProfile>("profile_duplicate", { profileId });

export const saveFolder = (request: { id?: string; name: string; parentId?: string }) =>
  invoke<ConnectionFolder>("folder_save", { request });

export const deleteFolder = (folderId: string, strategy: "move_to_default" | "delete_connections") =>
  invoke<void>("folder_delete", { request: { folderId, strategy } });

export const batchProfiles = (
  profileIds: string[],
  action: { type: "move"; folderId: string } | { type: "favorite"; favorite: boolean } | { type: "delete" },
) => invoke<ConnectionManagerSnapshot>("profiles_batch", { request: { profileIds, action } });

export const exportConnections = (path: string) =>
  invoke<void>("connections_export", { path });

export const previewConnectionsImport = (source: ImportSource, path: string) =>
  invoke<ImportPreview>("connections_import_preview", { source, path });

export const applyConnectionsImport = (source: ImportSource, path: string) =>
  invoke<ImportResult>("connections_import_apply", { source, path });

export const listSftpDirectory = (sessionId: string, path: string) =>
  invoke<DirectoryListing>("sftp_list", { sessionId, path });

export const listSftpDirectories = (sessionId: string, path: string) =>
  invoke<DirectoryTreeListing>("sftp_list_directories", { sessionId, path });

export const cancelSftpTransfer = (transferId: string) =>
  invoke<void>("sftp_cancel_transfer", { transferId });

export const listSftpTransferQueue = () =>
  invoke<TransferQueueSnapshot>("sftp_queue_list");

export const enqueueSftpTransfer = (request: {
  sessionId: string;
  serverLabel: string;
  direction: "upload" | "download";
  localPath: string;
  remotePath: string;
  conflictStrategy: ConflictStrategy;
  allowPause?: boolean;
}) => invoke<TransferQueueTask>("sftp_queue_enqueue", { request });

export const pauseSftpTransfer = (taskId: string) =>
  invoke<void>("sftp_queue_pause", { taskId });

export const resumeSftpTransfer = (taskId: string) =>
  invoke<void>("sftp_queue_resume", { taskId });

export const retrySftpTransfer = (taskId: string) =>
  invoke<void>("sftp_queue_retry", { taskId });

export const cancelQueuedSftpTransfer = (taskId: string, deletePartial: boolean) =>
  invoke<void>("sftp_queue_cancel", { taskId, deletePartial });

export const clearCompletedSftpTransfers = () =>
  invoke<void>("sftp_queue_clear_completed");

export const setSftpTransferConcurrency = (concurrency: number) =>
  invoke<void>("sftp_queue_set_concurrency", { concurrency });

export const wakeSftpTransferQueue = () =>
  invoke<void>("sftp_queue_wake");

export const listSftpDirectoryBatches = () =>
  invoke<DirectoryBatchSnapshot>("sftp_batch_list");

export const createSftpDirectoryBatch = (request: {
  sessionId: string;
  serverLabel: string;
  direction: "upload" | "download";
  sourceDirectory: string;
  targetDirectory: string;
  conflictStrategy: DirectoryConflictStrategy;
  directories: string[];
  fileCount: number;
}) => invoke<SftpDirectoryBatch>("sftp_batch_create", { request });

export const enqueueSftpDirectoryBatch = (
  batchId: string,
  requests: Array<{
    localPath: string;
    remotePath: string;
    conflictStrategy: ConflictStrategy;
  }>,
) => invoke<SftpDirectoryBatch>("sftp_batch_enqueue", { batchId, requests });

export const pauseSftpDirectoryBatch = (batchId: string) =>
  invoke<SftpDirectoryBatch>("sftp_batch_pause", { batchId });

export const resumeSftpDirectoryBatch = (batchId: string) =>
  invoke<SftpDirectoryBatch>("sftp_batch_resume", { batchId });

export const retrySftpDirectoryBatch = (batchId: string) =>
  invoke<SftpDirectoryBatch>("sftp_batch_retry", { batchId });

export const cancelSftpDirectoryBatch = (batchId: string, deletePartial: boolean) =>
  invoke<SftpDirectoryBatch>("sftp_batch_cancel", { batchId, deletePartial });

export const rollbackSftpDirectoryBatch = (batchId: string) =>
  invoke<SftpDirectoryBatch>("sftp_batch_rollback", { batchId });

export const deleteSftpDirectoryBatch = (batchId: string) =>
  invoke<void>("sftp_batch_delete", { batchId });

export const wakeSftpDirectoryBatches = () =>
  invoke<void>("sftp_batch_wake");

export const getLocalDirectoryManifest = (path: string, scanId: string) =>
  invoke<LocalDirectoryManifest>("sftp_local_directory_manifest", { path, scanId });

export const getRemoteDirectoryManifest = (sessionId: string, path: string, scanId: string) =>
  invoke<RemoteDirectoryManifest>("sftp_remote_directory_manifest", { sessionId, path, scanId });

export const inspectLocalPath = (path: string) =>
  invoke<LocalPathInspection>("sftp_inspect_local_path", { path });

export const inspectRemotePath = (sessionId: string, path: string) =>
  invoke<{ kind: LocalPathKind }>("sftp_inspect_remote_path", { sessionId, path });

export const createSftpDirectory = (sessionId: string, path: string) =>
  invoke<void>("sftp_create_directory", { sessionId, path });

export const renameSftpEntry = (sessionId: string, oldPath: string, newPath: string) =>
  invoke<void>("sftp_rename", { sessionId, oldPath, newPath });

export const deleteSftpEntry = (sessionId: string, path: string, isDirectory: boolean) =>
  invoke<void>("sftp_delete", { sessionId, path, isDirectory });

export const deleteSftpDirectoryRecursive = (sessionId: string, path: string) =>
  invoke<{ deletedFiles: number; deletedDirectories: number }>("sftp_delete_recursive", { sessionId, path });

export const listenSftpQueueTasks = (handler: (task: TransferQueueTask) => void): Promise<UnlistenFn> =>
  listen<TransferQueueTask>("sftp-queue-task", ({ payload }) => handler(payload));

export const listenSftpDirectoryBatches = (
  handler: (batch: SftpDirectoryBatch) => void,
): Promise<UnlistenFn> =>
  listen<SftpDirectoryBatch>("sftp-directory-batch", ({ payload }) => handler(payload));

export const fetchSystemMetrics = (sessionId: string) =>
  invoke<SystemMetrics>("system_metrics", { sessionId });

export const sendSshInput = (sessionId: string, data: string) =>
  invoke<void>("ssh_send", { sessionId, data });

export const resizeSsh = (sessionId: string, cols: number, rows: number) =>
  invoke<void>("ssh_resize", { sessionId, cols, rows });

export const disconnectSsh = (sessionId: string) =>
  invoke<void>("ssh_disconnect", { sessionId });

export const listenSshEvents = (handler: (event: SshEvent) => void): Promise<UnlistenFn> =>
  listen<SshEvent>("ssh-event", ({ payload }) => handler(payload));

export function commandErrorCode(error: unknown): string | undefined {
  if (typeof error === "object" && error !== null && "code" in error) {
    return String((error as CommandError).code);
  }
  return undefined;
}

export function describeCommandError(error: unknown): string {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as CommandError).message);
  }
  return error instanceof Error ? error.message : String(error);
}
