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

export type TransferCheckpoint = {
  version: number;
  taskId: string;
  sessionId: string;
  serverId: string;
  direction: "upload" | "download";
  sourcePath: string;
  targetPath: string;
  sourceSize: number;
  sourceModifiedAt?: number;
  sourceFingerprint: string;
  temporaryPath: string;
  transferred: number;
  createdAt: number;
  updatedAt: number;
  availableSessionId?: string;
};

export type TransferEvent = {
  transferId: string;
  sessionId: string;
  direction: "upload" | "download";
  fileName: string;
  transferred: number;
  total: number;
  state: "running" | "completed" | "failed" | "cancelled";
  message?: string;
  speedBytesPerSecond: number;
  etaSeconds?: number | null;
  resumedFrom: number;
};

export type ConflictStrategy = "overwrite" | "skip" | "rename";
export type DirectoryConflictStrategy = "merge" | "skip" | "rename" | "replace";
export type LocalPathKind = "missing" | "file" | "directory" | "symlink" | "other";

export type DirectoryPrepareResult = {
  path: string;
  skipped: boolean;
  existed: boolean;
  replacementId?: string;
};

export type TransferResult = {
  path: string;
  skipped: boolean;
  resumedFrom: number;
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

export const uploadSftpFile = (request: {
  sessionId: string;
  localPath: string;
  remotePath: string;
  transferId: string;
  taskId: string;
  conflictStrategy: ConflictStrategy;
  resume: boolean;
}) => invoke<TransferResult>("sftp_upload", request);

export const downloadSftpFile = (request: {
  sessionId: string;
  remotePath: string;
  localPath: string;
  transferId: string;
  taskId: string;
  conflictStrategy: ConflictStrategy;
  resume: boolean;
}) => invoke<TransferResult>("sftp_download", request);

export const cancelSftpTransfer = (transferId: string) =>
  invoke<void>("sftp_cancel_transfer", { transferId });

export const listSftpTransferCheckpoints = () =>
  invoke<TransferCheckpoint[]>("sftp_list_transfer_checkpoints");

export const deleteSftpTransferCheckpoint = (taskId: string) =>
  invoke<void>("sftp_delete_transfer_checkpoint", { taskId });

export const discardSftpTransferCheckpoint = (taskId: string, sessionId?: string) =>
  invoke<void>("sftp_discard_transfer_checkpoint", { taskId, sessionId });

export const getLocalDirectoryManifest = (path: string, scanId: string) =>
  invoke<LocalDirectoryManifest>("sftp_local_directory_manifest", { path, scanId });

export const getRemoteDirectoryManifest = (sessionId: string, path: string, scanId: string) =>
  invoke<RemoteDirectoryManifest>("sftp_remote_directory_manifest", { sessionId, path, scanId });

export const inspectLocalPath = (path: string) =>
  invoke<{ kind: LocalPathKind }>("sftp_inspect_local_path", { path });

export const inspectRemotePath = (sessionId: string, path: string) =>
  invoke<{ kind: LocalPathKind }>("sftp_inspect_remote_path", { sessionId, path });

export const prepareLocalDirectory = (
  path: string,
  conflictStrategy: DirectoryConflictStrategy = "merge",
  replacementId?: string,
) => invoke<DirectoryPrepareResult>("sftp_prepare_local_directory", { path, conflictStrategy, replacementId });

export const prepareRemoteDirectory = (
  sessionId: string,
  path: string,
  conflictStrategy: DirectoryConflictStrategy = "merge",
  replacementId?: string,
) => invoke<DirectoryPrepareResult>("sftp_prepare_remote_directory", {
  sessionId,
  path,
  conflictStrategy,
  replacementId,
});

export const finishDirectoryReplacement = (
  replacementId: string,
  commit: boolean,
  sessionId?: string,
) => invoke<void>("sftp_finish_directory_replacement", { replacementId, commit, sessionId });

export const createSftpDirectory = (sessionId: string, path: string) =>
  invoke<void>("sftp_create_directory", { sessionId, path });

export const renameSftpEntry = (sessionId: string, oldPath: string, newPath: string) =>
  invoke<void>("sftp_rename", { sessionId, oldPath, newPath });

export const deleteSftpEntry = (sessionId: string, path: string, isDirectory: boolean) =>
  invoke<void>("sftp_delete", { sessionId, path, isDirectory });

export const deleteSftpDirectoryRecursive = (sessionId: string, path: string) =>
  invoke<{ deletedFiles: number; deletedDirectories: number }>("sftp_delete_recursive", { sessionId, path });

export const listenSftpTransfers = (handler: (event: TransferEvent) => void): Promise<UnlistenFn> =>
  listen<TransferEvent>("sftp-transfer", ({ payload }) => handler(payload));

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
