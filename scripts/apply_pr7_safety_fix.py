from __future__ import annotations

from pathlib import Path


def read(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    Path(path).write_text(content, encoding="utf-8", newline="\n")


def replace_once(content: str, old: str, new: str, label: str) -> str:
    count = content.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    return content.replace(old, new, 1)


def replace_count(content: str, old: str, new: str, expected: int, label: str) -> str:
    count = content.count(old)
    if count != expected:
        raise RuntimeError(f"{label}: expected {expected} matches, found {count}")
    return content.replace(old, new)


def patch_ssh() -> None:
    path = "src-tauri/src/ssh.rs"
    text = read(path)

    text = replace_once(
        text,
        """struct SessionControl {
    commands: mpsc::Sender<SessionCommand>,
}
""",
        """#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionIdentity {
    host: String,
    port: u16,
    username: String,
    host_key_fingerprint: String,
}

impl SessionIdentity {
    fn new(host: &str, port: u16, username: &str, host_key_fingerprint: &str) -> Self {
        Self {
            host: host.trim().to_lowercase(),
            port,
            username: username.to_owned(),
            host_key_fingerprint: host_key_fingerprint.to_owned(),
        }
    }

    fn server_id(&self) -> String {
        format!(
            "v1|{}|{}|{}|{}",
            self.host, self.port, self.username, self.host_key_fingerprint
        )
    }
}

struct SessionControl {
    commands: mpsc::Sender<SessionCommand>,
    identity: SessionIdentity,
}
""",
        "session identity",
    )

    text = replace_once(
        text,
        """    Known,
    AcceptedNew {
""",
        """    Known {
        fingerprint: String,
    },
    AcceptedNew {
""",
        "known host observation",
    )

    text = replace_once(
        text,
        """                *self.observation.lock().expect("host observation poisoned") =
                    HostKeyObservation::Known;
""",
        """                *self.observation.lock().expect("host observation poisoned") =
                    HostKeyObservation::Known {
                        fingerprint: fingerprint.clone(),
                    };
""",
        "known host fingerprint",
    )

    text = replace_once(
        text,
        """    if let HostKeyObservation::AcceptedNew { fingerprint, key } = observation
        .lock()
        .expect("host observation poisoned")
        .clone()
    {
        if request.expected_host_fingerprint.as_deref() != Some(fingerprint.as_str()) {
            return Err(CommandError::host_key(
                "HOST_KEY_CONFIRMATION_MISMATCH",
                "确认的主机指纹与服务器返回值不一致",
                fingerprint,
            ));
        }
        learn_known_hosts_path(&request.host, request.port, &key, &known_hosts_path)
            .map_err(|error| CommandError::new("KNOWN_HOSTS_WRITE_FAILED", error.to_string()))?;
    }
""",
        """    let host_key_observation = observation
        .lock()
        .expect("host observation poisoned")
        .clone();
    let host_key_fingerprint = match &host_key_observation {
        HostKeyObservation::Known { fingerprint }
        | HostKeyObservation::AcceptedNew { fingerprint, .. } => fingerprint.clone(),
        _ => {
            return Err(CommandError::new(
                "HOST_KEY_IDENTITY_UNAVAILABLE",
                "无法确认当前 SSH 会话的服务器身份",
            ))
        }
    };
    if let HostKeyObservation::AcceptedNew { fingerprint, key } = host_key_observation {
        if request.expected_host_fingerprint.as_deref() != Some(fingerprint.as_str()) {
            return Err(CommandError::host_key(
                "HOST_KEY_CONFIRMATION_MISMATCH",
                "确认的主机指纹与服务器返回值不一致",
                fingerprint,
            ));
        }
        learn_known_hosts_path(&request.host, request.port, &key, &known_hosts_path)
            .map_err(|error| CommandError::new("KNOWN_HOSTS_WRITE_FAILED", error.to_string()))?;
    }
""",
        "derive verified host identity",
    )

    text = replace_once(
        text,
        """    let (commands, mut receiver) = mpsc::channel::<SessionCommand>(128);
    manager.sessions.write().await.insert(
        request.session_id.clone(),
        SessionControl {
            commands: commands.clone(),
        },
    );
""",
        """    let identity = SessionIdentity::new(
        &request.host,
        request.port,
        &request.username,
        &host_key_fingerprint,
    );
    let (commands, mut receiver) = mpsc::channel::<SessionCommand>(128);
    manager.sessions.write().await.insert(
        request.session_id.clone(),
        SessionControl {
            commands: commands.clone(),
            identity,
        },
    );
""",
        "store session identity",
    )

    text = replace_once(
        text,
        """pub(crate) async fn open_sftp(
""",
        """pub(crate) async fn session_server_id(
    manager: &SessionManager,
    session_id: &str,
) -> Result<String, CommandError> {
    manager
        .sessions
        .read()
        .await
        .get(session_id)
        .map(|control| control.identity.server_id())
        .ok_or_else(|| CommandError::new("SESSION_NOT_FOUND", "SSH 会话不存在或已经断开"))
}

pub(crate) async fn matching_session_id(
    manager: &SessionManager,
    server_id: &str,
) -> Option<String> {
    manager.sessions.read().await.iter().find_map(|(session_id, control)| {
        (control.identity.server_id() == server_id).then(|| session_id.clone())
    })
}

pub(crate) async fn open_sftp(
""",
        "session identity accessors",
    )

    text = replace_once(
        text,
        """    #[test]
    fn validates_connection_fields() {
        assert!(validate_request(&request("example.com", "root", 22)).is_ok());
        assert_eq!(
            validate_request(&request("", "root", 22)).unwrap_err().code,
            "INVALID_CONNECTION"
        );
        assert_eq!(
            validate_request(&request("example.com", "root", 0))
                .unwrap_err()
                .code,
            "INVALID_PORT"
        );
    }
}
""",
        """    #[test]
    fn validates_connection_fields() {
        assert!(validate_request(&request("example.com", "root", 22)).is_ok());
        assert_eq!(
            validate_request(&request("", "root", 22)).unwrap_err().code,
            "INVALID_CONNECTION"
        );
        assert_eq!(
            validate_request(&request("example.com", "root", 0))
                .unwrap_err()
                .code,
            "INVALID_PORT"
        );
    }

    #[test]
    fn derives_server_identity_from_verified_connection_details() {
        let first = SessionIdentity::new("EXAMPLE.com", 22, "root", "SHA256:first");
        let same = SessionIdentity::new("example.COM", 22, "root", "SHA256:first");
        let different_port = SessionIdentity::new("example.com", 2222, "root", "SHA256:first");
        let different_user = SessionIdentity::new("example.com", 22, "admin", "SHA256:first");
        let different_key = SessionIdentity::new("example.com", 22, "root", "SHA256:second");

        assert_eq!(first.server_id(), same.server_id());
        assert_ne!(first.server_id(), different_port.server_id());
        assert_ne!(first.server_id(), different_user.server_id());
        assert_ne!(first.server_id(), different_key.server_id());
    }
}
""",
        "session identity tests",
    )

    write(path, text)


def patch_sftp() -> None:
    path = "src-tauri/src/sftp.rs"
    text = read(path)

    text = replace_once(
        text,
        "use crate::ssh::{open_sftp, CommandError, SessionManager};",
        "use crate::ssh::{matching_session_id, open_sftp, session_server_id, CommandError, SessionManager};",
        "sftp ssh imports",
    )

    start = text.index("pub struct SftpTransferManager")
    end = text.index("#[derive(Debug, Serialize)]\n#[serde(rename_all = \"camelCase\")]\npub struct LocalDirectoryManifest")
    manager_block = """pub struct SftpTransferManager {
    cancelled: AsyncMutex<HashSet<String>>,
    active_targets: StdMutex<HashSet<TransferTargetKey>>,
    active_tasks: StdMutex<HashSet<String>>,
    slots: Semaphore,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum TransferDirection {
    Upload,
    Download,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TransferTargetKey {
    server_id: String,
    direction: TransferDirection,
    target_path: String,
}

impl TransferTargetKey {
    fn upload(server_id: &str, target_path: &str) -> Self {
        Self {
            server_id: server_id.to_owned(),
            direction: TransferDirection::Upload,
            target_path: normalize_remote_target(target_path),
        }
    }

    fn download(server_id: &str, target_path: &str) -> Self {
        Self {
            server_id: server_id.to_owned(),
            direction: TransferDirection::Download,
            target_path: normalize_local_target(target_path),
        }
    }
}

struct TransferTargetGuard<'a> {
    manager: &'a SftpTransferManager,
    key: Option<TransferTargetKey>,
}

impl Drop for TransferTargetGuard<'_> {
    fn drop(&mut self) {
        let Some(key) = self.key.take() else {
            return;
        };
        if let Ok(mut targets) = self.manager.active_targets.lock() {
            targets.remove(&key);
        }
    }
}

struct TransferTaskGuard<'a> {
    manager: &'a SftpTransferManager,
    task_id: Option<String>,
}

impl Drop for TransferTaskGuard<'_> {
    fn drop(&mut self) {
        let Some(task_id) = self.task_id.take() else {
            return;
        };
        if let Ok(mut tasks) = self.manager.active_tasks.lock() {
            tasks.remove(&task_id);
        }
    }
}

impl SftpTransferManager {
    fn acquire_target(
        &self,
        key: TransferTargetKey,
    ) -> Result<TransferTargetGuard<'_>, CommandError> {
        let mut targets = self.active_targets.lock().map_err(|_| {
            CommandError::new(
                "TRANSFER_TARGET_LOCK_FAILED",
                "传输目标锁不可用，请稍后重试",
            )
        })?;
        if !targets.insert(key.clone()) {
            return Err(CommandError::new(
                "TRANSFER_TARGET_BUSY",
                "该目标文件已有传输任务正在运行",
            ));
        }
        Ok(TransferTargetGuard {
            manager: self,
            key: Some(key),
        })
    }

    fn acquire_task(&self, task_id: &str) -> Result<TransferTaskGuard<'_>, CommandError> {
        let mut tasks = self.active_tasks.lock().map_err(|_| {
            CommandError::new(
                "TRANSFER_TASK_LOCK_FAILED",
                "传输任务锁不可用，请稍后重试",
            )
        })?;
        if !tasks.insert(task_id.to_owned()) {
            return Err(CommandError::new(
                "TRANSFER_TASK_BUSY",
                "该传输任务已经在运行",
            ));
        }
        Ok(TransferTaskGuard {
            manager: self,
            task_id: Some(task_id.to_owned()),
        })
    }
}

impl Default for SftpTransferManager {
    fn default() -> Self {
        Self {
            cancelled: AsyncMutex::new(HashSet::new()),
            active_targets: StdMutex::new(HashSet::new()),
            active_tasks: StdMutex::new(HashSet::new()),
            slots: Semaphore::new(3),
        }
    }
}

"""
    text = text[:start] + manager_block + text[end:]

    start = text.index("#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]\n#[serde(rename_all = \"camelCase\")]\npub struct TransferCheckpoint")
    end = text.index("#[tauri::command]\npub async fn sftp_cancel_transfer")
    checkpoint_block = """const CHECKPOINT_VERSION: u8 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TransferCheckpoint {
    version: u8,
    task_id: String,
    session_id: String,
    server_id: String,
    direction: String,
    source_path: String,
    target_path: String,
    source_size: u64,
    source_modified_at: Option<u64>,
    #[serde(default)]
    source_fingerprint: String,
    temporary_path: String,
    transferred: u64,
    created_at: u64,
    updated_at: u64,
    #[serde(skip_deserializing, skip_serializing_if = "Option::is_none")]
    available_session_id: Option<String>,
}

impl TransferCheckpoint {
    #[allow(clippy::too_many_arguments)]
    fn new(
        task_id: &str,
        session_id: &str,
        server_id: &str,
        direction: &str,
        source_path: &str,
        target_path: &str,
        source_size: u64,
        source_modified_at: Option<u64>,
        source_fingerprint: &str,
        temporary_path: &str,
    ) -> Self {
        let now = unix_now();
        Self {
            version: CHECKPOINT_VERSION,
            task_id: task_id.to_owned(),
            session_id: session_id.to_owned(),
            server_id: server_id.to_owned(),
            direction: direction.to_owned(),
            source_path: source_path.to_owned(),
            target_path: target_path.to_owned(),
            source_size,
            source_modified_at,
            source_fingerprint: source_fingerprint.to_owned(),
            temporary_path: temporary_path.to_owned(),
            transferred: 0,
            created_at: now,
            updated_at: now,
            available_session_id: None,
        }
    }
}

"""
    text = text[:start] + checkpoint_block + text[end:]

    text = replace_count(
        text,
        """    task_id: String,
    server_id: String,
    conflict_strategy: ConflictStrategy,
""",
        """    task_id: String,
    conflict_strategy: ConflictStrategy,
""",
        2,
        "remove renderer server ids",
    )
    text = replace_count(
        text,
        """    validate_transfer(&local_path, &remote_path, &transfer_id, &task_id)?;
    validate_server_id(&server_id)?;
""",
        """    validate_transfer(&local_path, &remote_path, &transfer_id, &task_id)?;
""",
        2,
        "remove renderer server validation",
    )
    text = replace_count(
        text,
        """        .await
        .map_err(|_| CommandError::new("TRANSFER_QUEUE_CLOSED", "传输队列已经关闭"))?;
    let sftp = open_sftp(&manager, &session_id).await?;
""",
        """        .await
        .map_err(|_| CommandError::new("TRANSFER_QUEUE_CLOSED", "传输队列已经关闭"))?;
    let _task_guard = transfers.acquire_task(&task_id)?;
    let server_id = session_server_id(&manager, &session_id).await?;
    let sftp = open_sftp(&manager, &session_id).await?;
""",
        2,
        "derive backend server identity",
    )
    text = text.replace(
        "TransferTargetKey::upload(&session_id, &target_path)",
        "TransferTargetKey::upload(&server_id, &target_path)",
    )
    text = text.replace(
        "TransferTargetKey::download(&session_id, &target_path)",
        "TransferTargetKey::download(&server_id, &target_path)",
    )

    text = replace_once(
        text,
        """    let total = source_metadata.len();
    let source_modified_at = modified_seconds(source_metadata.modified().ok());
    transfers.cancelled.lock().await.remove(&transfer_id);
""",
        """    let total = source_metadata.len();
    let source_modified_at = modified_nanos(source_metadata.modified().ok());
    let source_fingerprint = source_sample_fingerprint(&mut source, total).await?;
    transfers.cancelled.lock().await.remove(&transfer_id);
""",
        "upload source identity",
    )
    text = replace_once(
        text,
        """    let total = source_metadata.len();
    let source_modified_at = modified_seconds(source_metadata.modified().ok());
    let mut source = sftp
        .open(remote_path.clone())
        .await
        .map_err(sftp_error("SFTP_DOWNLOAD_OPEN_FAILED"))?;
    transfers.cancelled.lock().await.remove(&transfer_id);
""",
        """    let total = source_metadata.len();
    let source_modified_at = modified_nanos(source_metadata.modified().ok());
    let mut source = sftp
        .open(remote_path.clone())
        .await
        .map_err(sftp_error("SFTP_DOWNLOAD_OPEN_FAILED"))?;
    let source_fingerprint = source_sample_fingerprint(&mut source, total).await?;
    transfers.cancelled.lock().await.remove(&transfer_id);
""",
        "download source identity",
    )
    text = replace_count(
        text,
        """        total,
        source_modified_at,
        &normalize_""",
        """        total,
        source_modified_at,
        &source_fingerprint,
        &normalize_""",
        2,
        "checkpoint source fingerprint",
    )

    text = replace_once(
        text,
        """pub async fn sftp_list_transfer_checkpoints(
    app: AppHandle,
) -> Result<Vec<TransferCheckpoint>, CommandError> {
""",
        """pub async fn sftp_list_transfer_checkpoints(
    app: AppHandle,
    manager: State<'_, SessionManager>,
) -> Result<Vec<TransferCheckpoint>, CommandError> {
""",
        "checkpoint session availability",
    )
    text = replace_once(
        text,
        """        if let Ok(checkpoint) = serde_json::from_slice::<TransferCheckpoint>(&content) {
            if validate_task_id(&checkpoint.task_id).is_ok() {
                checkpoints.push(checkpoint);
            }
        }
""",
        """        if let Ok(mut checkpoint) = serde_json::from_slice::<TransferCheckpoint>(&content) {
            if validate_task_id(&checkpoint.task_id).is_ok() {
                checkpoint.available_session_id =
                    matching_session_id(&manager, &checkpoint.server_id).await;
                checkpoints.push(checkpoint);
            }
        }
""",
        "resolve checkpoint sessions",
    )

    text = replace_once(
        text,
        """    task_id: String,
    session_id: Option<String>,
    server_id: Option<String>,
) -> Result<(), CommandError> {
""",
        """    task_id: String,
    session_id: Option<String>,
) -> Result<(), CommandError> {
""",
        "discard command arguments",
    )
    text = replace_once(
        text,
        """    if checkpoint.direction == "upload" {
        if server_id.as_deref() != Some(checkpoint.server_id.as_str()) {
            return Err(CommandError::new(
                "TRANSFER_DISCARD_SERVER_MISMATCH",
                "当前连接与检查点所属服务器不匹配",
            ));
        }
        let session_id = session_id.ok_or_else(|| {
            CommandError::new(
                "TRANSFER_DISCARD_SESSION_REQUIRED",
                "删除远程临时文件前请重新连接对应服务器",
            )
        })?;
        let sftp = open_sftp(&manager, &session_id).await?;
""",
        """    if checkpoint.direction == "upload" {
        let session_id = session_id.ok_or_else(|| {
            CommandError::new(
                "TRANSFER_DISCARD_SESSION_REQUIRED",
                "删除远程临时文件前请重新连接对应服务器",
            )
        })?;
        let current_server_id = session_server_id(&manager, &session_id).await?;
        if current_server_id != checkpoint.server_id {
            return Err(CommandError::new(
                "TRANSFER_DISCARD_SERVER_MISMATCH",
                "当前 SSH 会话与检查点所属服务器不匹配",
            ));
        }
        let sftp = open_sftp(&manager, &session_id).await?;
""",
        "verify discard session identity",
    )

    text = replace_once(
        text,
        """        && saved.source_modified_at == expected.source_modified_at
        && saved.temporary_path == expected.temporary_path;
""",
        """        && saved.source_modified_at == expected.source_modified_at
        && saved.source_fingerprint == expected.source_fingerprint
        && saved.temporary_path == expected.temporary_path;
""",
        "resume fingerprint validation",
    )

    start = text.index("fn modified_seconds(value: Option<std::time::SystemTime>)")
    end = text.index("fn unix_now()", start)
    fingerprint_helpers = """const SOURCE_SAMPLE_SIZE: u64 = 64 * 1024;
const FNV_OFFSET_1: u64 = 0xcbf29ce484222325;
const FNV_OFFSET_2: u64 = 0x84222325cbf29ce4;
const FNV_PRIME_1: u64 = 0x100000001b3;
const FNV_PRIME_2: u64 = 0x9e3779b185ebca87;

async fn source_sample_fingerprint<R>(
    source: &mut R,
    total: u64,
) -> Result<String, CommandError>
where
    R: AsyncRead + AsyncSeek + Unpin,
{
    let mut first = FNV_OFFSET_1;
    let mut second = FNV_OFFSET_2;
    update_sample_hash(&mut first, &total.to_le_bytes(), FNV_PRIME_1);
    update_sample_hash(&mut second, &total.to_le_bytes(), FNV_PRIME_2);

    let samples = if total <= SOURCE_SAMPLE_SIZE * 2 {
        vec![(0, total)]
    } else {
        vec![
            (0, SOURCE_SAMPLE_SIZE),
            (total - SOURCE_SAMPLE_SIZE, SOURCE_SAMPLE_SIZE),
        ]
    };
    let mut buffer = vec![0_u8; SOURCE_SAMPLE_SIZE as usize];
    for (offset, length) in samples {
        source
            .seek(SeekFrom::Start(offset))
            .await
            .map_err(|error| CommandError::new("TRANSFER_SOURCE_FINGERPRINT_FAILED", error.to_string()))?;
        update_sample_hash(&mut first, &offset.to_le_bytes(), FNV_PRIME_1);
        update_sample_hash(&mut second, &offset.to_le_bytes(), FNV_PRIME_2);
        let mut remaining = length;
        while remaining > 0 {
            let requested = remaining.min(buffer.len() as u64) as usize;
            let read = source
                .read(&mut buffer[..requested])
                .await
                .map_err(|error| CommandError::new("TRANSFER_SOURCE_FINGERPRINT_FAILED", error.to_string()))?;
            if read == 0 {
                return Err(CommandError::new(
                    "TRANSFER_SOURCE_FINGERPRINT_FAILED",
                    "读取源文件采样内容时提前结束",
                ));
            }
            update_sample_hash(&mut first, &buffer[..read], FNV_PRIME_1);
            update_sample_hash(&mut second, &buffer[..read], FNV_PRIME_2);
            remaining -= read as u64;
        }
    }
    source
        .seek(SeekFrom::Start(0))
        .await
        .map_err(|error| CommandError::new("TRANSFER_SOURCE_FINGERPRINT_FAILED", error.to_string()))?;
    Ok(format!("{first:016x}{second:016x}"))
}

fn update_sample_hash(hash: &mut u64, bytes: &[u8], prime: u64) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(prime);
    }
}

fn modified_nanos(value: Option<std::time::SystemTime>) -> Option<u64> {
    value
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|duration| u64::try_from(duration.as_nanos()).ok())
}

"""
    text = text[:start] + fingerprint_helpers + text[end:]

    validation_start = text.index("fn validate_server_id(server_id: &str)")
    validation_end = text.index("fn validate_task_id(task_id: &str)", validation_start)
    text = text[:validation_start] + text[validation_end:]

    text = text.replace(
        'TransferTargetKey::upload("session-a",',
        'TransferTargetKey::upload("server-a",',
    )
    text = text.replace(
        'TransferTargetKey::download("session-a",',
        'TransferTargetKey::download("server-a",',
    )
    text = replace_count(
        text,
        """            Some(10),
            "/tmp/target.txt.liteshell-task-1.part",
""",
        """            Some(10),
            "fingerprint-a",
            "/tmp/target.txt.liteshell-task-1.part",
""",
        1,
        "resume test fingerprint",
    )
    text = replace_count(
        text,
        """            Some(1),
            "C:\\tmp\\file.txt.liteshell-task-1.part",
""",
        """            Some(1),
            "fingerprint-a",
            "C:\\tmp\\file.txt.liteshell-task-1.part",
""",
        1,
        "path test fingerprint",
    )
    text = replace_once(
        text,
        """        let mut changed = expected.clone();
        changed.source_modified_at = Some(11);
""",
        """        let mut changed = expected.clone();
        changed.source_modified_at = Some(11);
""",
        "keep modification identity test",
    )
    text = replace_once(
        text,
        """        assert_eq!(
            validate_resume_checkpoint(&saved, &expected, 101)
                .unwrap_err()
                .code,
            "TRANSFER_RESUME_CHECKPOINT_INVALID"
        );
    }

    #[test]
    fn validates_transfer_task_ids() {
""",
        """        let mut changed_content = expected.clone();
        changed_content.source_fingerprint = "fingerprint-b".to_owned();
        assert_eq!(
            validate_resume_checkpoint(&saved, &changed_content, 75)
                .unwrap_err()
                .code,
            "TRANSFER_RESUME_SOURCE_CHANGED"
        );
        assert_eq!(
            validate_resume_checkpoint(&saved, &expected, 101)
                .unwrap_err()
                .code,
            "TRANSFER_RESUME_CHECKPOINT_INVALID"
        );
    }

    #[test]
    fn locks_a_stable_task_id_until_guard_drops() {
        let manager = SftpTransferManager::default();
        let first = manager.acquire_task("task-a").unwrap();
        assert_eq!(
            manager.acquire_task("task-a").unwrap_err().code,
            "TRANSFER_TASK_BUSY"
        );
        drop(first);
        assert!(manager.acquire_task("task-a").is_ok());
    }

    #[tokio::test]
    async fn sampled_fingerprint_detects_same_size_content_changes() {
        let mut first = std::io::Cursor::new(vec![1_u8; 256 * 1024]);
        let mut second_data = vec![1_u8; 256 * 1024];
        second_data[0] = 2;
        second_data[second_data.len() - 1] = 3;
        let mut second = std::io::Cursor::new(second_data);

        let first_fingerprint = source_sample_fingerprint(&mut first, 256 * 1024)
            .await
            .unwrap();
        let second_fingerprint = source_sample_fingerprint(&mut second, 256 * 1024)
            .await
            .unwrap();
        assert_ne!(first_fingerprint, second_fingerprint);
    }

    #[test]
    fn validates_transfer_task_ids() {
""",
        "content and task lock tests",
    )

    write(path, text)


def patch_frontend() -> None:
    app_path = "src/App.vue"
    app = read(app_path)
    app = replace_once(
        app,
        """type TransferTask =
  | { taskId: string; serverId: string; direction: "upload"; sessionId: string; localPath: string; remotePath: string; conflictStrategy: ConflictStrategy; resume: boolean }
  | { taskId: string; serverId: string; direction: "download"; sessionId: string; remotePath: string; localPath: string; conflictStrategy: ConflictStrategy; resume: boolean };
""",
        """type TransferTask =
  | { taskId: string; direction: "upload"; sessionId: string; localPath: string; remotePath: string; conflictStrategy: ConflictStrategy; resume: boolean }
  | { taskId: string; direction: "download"; sessionId: string; remotePath: string; localPath: string; conflictStrategy: ConflictStrategy; resume: boolean };
""",
        "frontend task identity",
    )
    app = replace_once(
        app,
        """const serverIdForSession = (session: Session) => session.profileId ?? session.name;
const checkpointSession = (checkpoint: TransferCheckpoint) =>
  sessions.value.find((session) => session.connected && serverIdForSession(session) === checkpoint.serverId);
""",
        """const checkpointSession = (checkpoint: TransferCheckpoint) =>
  sessions.value.find((session) => session.connected && session.id === checkpoint.availableSessionId);
""",
        "backend checkpoint session match",
    )
    app = replace_once(
        app,
        """      const taskId = crypto.randomUUID();
      const serverId = serverIdForSession(session);
      const request = { taskId, serverId, direction: "upload" as const, sessionId, ...task, resume: false };
      transferTasks.set(transferId, request);
      const result = await uploadSftpFile({ sessionId, transferId, taskId, serverId, ...task, resume: false });
""",
        """      const taskId = crypto.randomUUID();
      const request = { taskId, direction: "upload" as const, sessionId, ...task, resume: false };
      transferTasks.set(transferId, request);
      const result = await uploadSftpFile({ sessionId, transferId, taskId, ...task, resume: false });
""",
        "file upload server identity",
    )
    app = replace_once(
        app,
        """      const taskId = crypto.randomUUID();
      const serverId = serverIdForSession(session);
      const remotePath = joinRemotePath(remoteRoot, file.relativePath);
      const request = { taskId, serverId, direction: "upload" as const, sessionId, localPath: file.absolutePath, remotePath, conflictStrategy, resume: false };
      transferTasks.set(transferId, request);
      const result = await uploadSftpFile({ sessionId, localPath: file.absolutePath, remotePath, transferId, taskId, serverId, conflictStrategy, resume: false });
""",
        """      const taskId = crypto.randomUUID();
      const remotePath = joinRemotePath(remoteRoot, file.relativePath);
      const request = { taskId, direction: "upload" as const, sessionId, localPath: file.absolutePath, remotePath, conflictStrategy, resume: false };
      transferTasks.set(transferId, request);
      const result = await uploadSftpFile({ sessionId, localPath: file.absolutePath, remotePath, transferId, taskId, conflictStrategy, resume: false });
""",
        "directory upload server identity",
    )
    app = replace_once(
        app,
        """  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session) return;
  const serverId = serverIdForSession(session);
  const request = { taskId, serverId, direction: "download" as const, sessionId, remotePath, localPath, conflictStrategy, resume: false };
  transferTasks.set(transferId, request);
  const result = await downloadSftpFile({ sessionId, remotePath, localPath, transferId, taskId, serverId, conflictStrategy, resume: false });
""",
        """  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session) return;
  const request = { taskId, direction: "download" as const, sessionId, remotePath, localPath, conflictStrategy, resume: false };
  transferTasks.set(transferId, request);
  const result = await downloadSftpFile({ sessionId, remotePath, localPath, transferId, taskId, conflictStrategy, resume: false });
""",
        "download server identity",
    )
    app = replace_once(
        app,
        """  const common = {
    taskId: checkpoint.taskId,
    serverId: checkpoint.serverId,
    sessionId: session.id,
""",
        """  const common = {
    taskId: checkpoint.taskId,
    sessionId: session.id,
""",
        "recovered transfer identity",
    )
    app = replace_once(
        app,
        """    await discardSftpTransferCheckpoint(checkpoint.taskId, session?.id, session ? serverIdForSession(session) : undefined);
""",
        """    await discardSftpTransferCheckpoint(checkpoint.taskId, session?.id);
""",
        "discard identity",
    )
    write(app_path, app)

    service_path = "src/services/ssh.ts"
    service = read(service_path)
    service = replace_once(
        service,
        """  sourceModifiedAt?: number;
  temporaryPath: string;
""",
        """  sourceModifiedAt?: number;
  sourceFingerprint: string;
  temporaryPath: string;
""",
        "checkpoint fingerprint type",
    )
    service = replace_once(
        service,
        """  updatedAt: number;
};
""",
        """  updatedAt: number;
  availableSessionId?: string;
};
""",
        "checkpoint session type",
    )
    service = replace_count(
        service,
        """  taskId: string;
  serverId: string;
  conflictStrategy: ConflictStrategy;
""",
        """  taskId: string;
  conflictStrategy: ConflictStrategy;
""",
        2,
        "remove renderer server id types",
    )
    service = replace_once(
        service,
        """export const discardSftpTransferCheckpoint = (taskId: string, sessionId?: string, serverId?: string) =>
  invoke<void>("sftp_discard_transfer_checkpoint", { taskId, sessionId, serverId });
""",
        """export const discardSftpTransferCheckpoint = (taskId: string, sessionId?: string) =>
  invoke<void>("sftp_discard_transfer_checkpoint", { taskId, sessionId });
""",
        "discard service identity",
    )
    write(service_path, service)


def patch_docs() -> None:
    readme_path = "README.md"
    readme = read(readme_path)
    readme = replace_once(
        readme,
        "- 断点续传使用稳定任务 ID 和持久检查点，源身份不匹配时拒绝拼接。",
        "- 断点续传使用稳定任务 ID、后端验证的 SSH 服务器身份和持久检查点，源身份不匹配时拒绝拼接。",
        "readme checkpoint capability",
    )
    readme = replace_once(
        readme,
        "- 断点续传会校验源路径、目标路径、服务器标识、大小和修改时间；尚未加入全文件内容哈希。",
        "- 断点续传会校验后端 SSH 身份、源/目标路径、大小、纳秒级修改时间和首尾内容采样指纹；尚未执行全文件哈希。",
        "readme checkpoint limitation",
    )
    write(readme_path, readme)

    plan_path = "plan.md"
    plan = read(plan_path)
    plan = replace_once(
        plan,
        "- Tauri 应用数据目录持久化版本化检查点，包含稳定服务器标识、会话、方向、源/目标、源大小、修改时间、taskId 绑定临时路径、已传输字节和时间戳。",
        "- Tauri 应用数据目录持久化版本化检查点，服务器标识由 Rust 根据 host、port、username 和已验证主机密钥派生，前端不能伪造。",
        "plan backend identity",
    )
    plan = replace_once(
        plan,
        "- 续传必须通过完整身份校验；源或目标变化、检查点损坏、临时文件缺失/超长都会拒绝续传。",
        "- 续传校验源/目标、后端服务器身份、大小、纳秒级修改时间和首尾采样指纹；同大小内容变化、错误会话、检查点损坏及临时文件异常都会被拒绝。",
        "plan fingerprint validation",
    )
    plan = replace_once(
        plan,
        "- 新增检查点身份、临时路径绑定和任务 ID 测试。",
        "- 新增会话身份区分、稳定任务互斥、检查点身份、内容采样指纹和临时路径绑定测试。",
        "plan tests",
    )
    write(plan_path, plan)

    handoff_path = "handoff.md"
    handoff = read(handoff_path)
    handoff = replace_once(
        handoff,
        "- PR5：稳定 taskId、应用数据目录检查点和源身份校验，拒绝不安全续传。",
        "- PR5：稳定 taskId、后端验证的 SSH 身份、应用数据目录检查点和内容采样指纹，拒绝不安全续传。",
        "handoff capability",
    )
    handoff = replace_once(
        handoff,
        "5. PR5 已实现稳定 taskId、源身份校验、持久检查点和重启识别；尚未加入全文件内容哈希。",
        "5. PR5 已实现稳定 taskId、后端会话身份、纳秒时间、内容采样指纹、持久检查点和重启识别；尚未执行全文件哈希。",
        "handoff risk",
    )
    handoff = replace_once(
        handoff,
        "- 稳定 `taskId` 与每次尝试的 `transferId` 分离。",
        "- 稳定 `taskId` 与每次尝试的 `transferId` 分离，并阻止同一 taskId 并发运行。",
        "handoff task lock",
    )
    handoff = replace_once(
        handoff,
        "- `.part` 文件名绑定 `taskId`。",
        "- `.part` 文件名绑定 `taskId`，同一服务器不同 SSH 会话共享目标互斥。",
        "handoff target lock",
    )
    write(handoff_path, handoff)


def main() -> None:
    patch_ssh()
    patch_sftp()
    patch_frontend()
    patch_docs()
    Path("scripts/apply_pr7_safety_fix.py").unlink()
    Path(".github/workflows/apply-pr7-safety-fix.yml").unlink()


if __name__ == "__main__":
    main()
