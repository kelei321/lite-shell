use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex as StdMutex},
};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use russh::{
    client,
    keys::{
        known_hosts::{check_known_hosts_path, learn_known_hosts_path},
        load_secret_key, ssh_key, PrivateKeyWithHashAlg,
    },
    ChannelMsg, Disconnect,
};
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{mpsc, oneshot, RwLock};

use crate::profiles::{resolve_profile, ProfileAuthType};

#[derive(Default)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, SessionControl>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

enum SessionCommand {
    Input(Vec<u8>),
    Resize {
        cols: u32,
        rows: u32,
    },
    OpenSftp(oneshot::Sender<Result<SftpSession, String>>),
    OpenExec {
        command: String,
        reply: oneshot::Sender<Result<russh::Channel<client::Msg>, String>>,
    },
    Disconnect,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectRequest {
    session_id: String,
    host: String,
    port: u16,
    username: String,
    auth: AuthMethod,
    cols: u32,
    rows: u32,
    expected_host_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileConnectRequest {
    profile_id: String,
    session_id: String,
    cols: u32,
    rows: u32,
    expected_host_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthMethod {
    Password {
        password: String,
    },
    PrivateKey {
        path: String,
        passphrase: Option<String>,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ConnectOutcome {
    Connected {
        session_id: String,
        host: String,
        port: u16,
        username: String,
    },
    HostKeyConfirmationRequired {
        fingerprint: String,
        algorithm: String,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SshEvent {
    session_id: String,
    kind: &'static str,
    data_base64: Option<String>,
    message: Option<String>,
    exit_status: Option<u32>,
}

impl SshEvent {
    fn state(session_id: &str, kind: &'static str) -> Self {
        Self {
            session_id: session_id.to_owned(),
            kind,
            data_base64: None,
            message: None,
            exit_status: None,
        }
    }

    fn output(session_id: &str, data: &[u8]) -> Self {
        Self {
            session_id: session_id.to_owned(),
            kind: "data",
            data_base64: Some(BASE64.encode(data)),
            message: None,
            exit_status: None,
        }
    }

    fn error(session_id: &str, message: impl Into<String>) -> Self {
        Self {
            session_id: session_id.to_owned(),
            kind: "error",
            data_base64: None,
            message: Some(message.into()),
            exit_status: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    fingerprint: Option<String>,
}

impl CommandError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            fingerprint: None,
        }
    }

    fn host_key(code: &'static str, message: impl Into<String>, fingerprint: String) -> Self {
        Self {
            code,
            message: message.into(),
            fingerprint: Some(fingerprint),
        }
    }
}

#[derive(Clone)]
struct HostKeyHandler {
    host: String,
    port: u16,
    known_hosts_path: PathBuf,
    expected_fingerprint: Option<String>,
    observation: Arc<StdMutex<HostKeyObservation>>,
}

#[derive(Debug, Clone, Default)]
enum HostKeyObservation {
    #[default]
    None,
    Known {
        fingerprint: String,
    },
    AcceptedNew {
        fingerprint: String,
        key: ssh_key::PublicKey,
    },
    Unknown {
        fingerprint: String,
        algorithm: String,
    },
    Changed {
        fingerprint: String,
    },
    VerificationFailed(String),
}

impl client::Handler for HostKeyHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = server_public_key
            .fingerprint(ssh_key::HashAlg::Sha256)
            .to_string();

        match check_known_hosts_path(
            &self.host,
            self.port,
            server_public_key,
            &self.known_hosts_path,
        ) {
            Ok(true) => {
                *self.observation.lock().expect("host observation poisoned") =
                    HostKeyObservation::Known {
                        fingerprint: fingerprint.clone(),
                    };
                Ok(true)
            }
            Ok(false) => {
                if self.expected_fingerprint.as_deref() == Some(fingerprint.as_str()) {
                    *self.observation.lock().expect("host observation poisoned") =
                        HostKeyObservation::AcceptedNew {
                            fingerprint,
                            key: server_public_key.clone(),
                        };
                    Ok(true)
                } else {
                    *self.observation.lock().expect("host observation poisoned") =
                        HostKeyObservation::Unknown {
                            fingerprint,
                            algorithm: server_public_key.algorithm().to_string(),
                        };
                    Ok(false)
                }
            }
            Err(russh::keys::Error::KeyChanged { .. }) => {
                *self.observation.lock().expect("host observation poisoned") =
                    HostKeyObservation::Changed { fingerprint };
                Ok(false)
            }
            Err(error) => {
                *self.observation.lock().expect("host observation poisoned") =
                    HostKeyObservation::VerificationFailed(error.to_string());
                Ok(false)
            }
        }
    }
}

#[tauri::command]
pub async fn ssh_connect(
    app: AppHandle,
    manager: State<'_, SessionManager>,
    request: ConnectRequest,
) -> Result<ConnectOutcome, CommandError> {
    connect_inner(app, &manager, request).await
}

#[tauri::command]
pub async fn ssh_connect_profile(
    app: AppHandle,
    manager: State<'_, SessionManager>,
    request: ProfileConnectRequest,
) -> Result<ConnectOutcome, CommandError> {
    let resolved = resolve_profile(&app, request.profile_id).await?;
    let auth = match resolved.profile.auth_type {
        ProfileAuthType::Password => AuthMethod::Password {
            password: resolved.secret.ok_or_else(|| {
                CommandError::new("CREDENTIAL_REQUIRED", "该连接没有保存密码，请编辑后重试")
            })?,
        },
        ProfileAuthType::PrivateKey => AuthMethod::PrivateKey {
            path: resolved.profile.private_key_path.unwrap_or_default(),
            passphrase: resolved.secret,
        },
    };
    connect_inner(
        app,
        &manager,
        ConnectRequest {
            session_id: request.session_id,
            host: resolved.profile.host,
            port: resolved.profile.port,
            username: resolved.profile.username,
            auth,
            cols: request.cols,
            rows: request.rows,
            expected_host_fingerprint: request.expected_host_fingerprint,
        },
    )
    .await
}

async fn connect_inner(
    app: AppHandle,
    manager: &SessionManager,
    request: ConnectRequest,
) -> Result<ConnectOutcome, CommandError> {
    validate_request(&request)?;

    if manager
        .sessions
        .read()
        .await
        .contains_key(&request.session_id)
    {
        return Err(CommandError::new(
            "SESSION_EXISTS",
            "该会话已经连接或正在连接",
        ));
    }

    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|error| CommandError::new("APP_DATA_UNAVAILABLE", error.to_string()))?;
    let known_hosts_path = app_data.join("ssh").join("known_hosts");
    let observation = Arc::new(StdMutex::new(HostKeyObservation::None));
    let handler = HostKeyHandler {
        host: request.host.clone(),
        port: request.port,
        known_hosts_path: known_hosts_path.clone(),
        expected_fingerprint: request.expected_host_fingerprint.clone(),
        observation: observation.clone(),
    };

    app.emit(
        "ssh-event",
        SshEvent::state(&request.session_id, "connecting"),
    )
    .ok();

    let config = Arc::new(client::Config {
        inactivity_timeout: None,
        ..Default::default()
    });
    let address = format!("{}:{}", request.host, request.port);
    let mut handle = match client::connect(config, address, handler).await {
        Ok(handle) => handle,
        Err(error) => {
            let observed = observation
                .lock()
                .expect("host observation poisoned")
                .clone();
            return match observed {
                HostKeyObservation::Unknown {
                    fingerprint,
                    algorithm,
                } => Ok(ConnectOutcome::HostKeyConfirmationRequired {
                    fingerprint,
                    algorithm,
                }),
                HostKeyObservation::Changed { fingerprint } => Err(CommandError::host_key(
                    "HOST_KEY_CHANGED",
                    "服务器主机密钥与已保存记录不一致，连接已阻止",
                    fingerprint,
                )),
                HostKeyObservation::VerificationFailed(message) => {
                    Err(CommandError::new("HOST_KEY_VERIFICATION_FAILED", message))
                }
                _ => Err(CommandError::new("CONNECT_FAILED", error.to_string())),
            };
        }
    };

    authenticate(&mut handle, &request.username, &request.auth).await?;

    let host_key_observation = observation
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

    let mut channel = handle
        .channel_open_session()
        .await
        .map_err(|error| CommandError::new("CHANNEL_OPEN_FAILED", error.to_string()))?;
    channel
        .request_pty(
            false,
            "xterm-256color",
            request.cols.clamp(20, 500),
            request.rows.clamp(5, 300),
            0,
            0,
            &[],
        )
        .await
        .map_err(|error| CommandError::new("PTY_REQUEST_FAILED", error.to_string()))?;
    channel
        .request_shell(true)
        .await
        .map_err(|error| CommandError::new("SHELL_REQUEST_FAILED", error.to_string()))?;

    let identity = SessionIdentity::new(
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

    let sessions = manager.sessions.clone();
    let app_for_task = app.clone();
    let session_id_for_task = request.session_id.clone();
    tokio::spawn(async move {
        let _ = app_for_task.emit(
            "ssh-event",
            SshEvent::state(&session_id_for_task, "connected"),
        );

        loop {
            tokio::select! {
                command = receiver.recv() => {
                    match command {
                        Some(SessionCommand::Input(data)) => {
                            if let Err(error) = channel.data(data.as_slice()).await {
                                let _ = app_for_task.emit("ssh-event", SshEvent::error(&session_id_for_task, error.to_string()));
                                break;
                            }
                        }
                        Some(SessionCommand::Resize { cols, rows }) => {
                            if let Err(error) = channel.window_change(cols, rows, 0, 0).await {
                                let _ = app_for_task.emit("ssh-event", SshEvent::error(&session_id_for_task, error.to_string()));
                            }
                        }
                        Some(SessionCommand::OpenSftp(reply)) => {
                            let result = async {
                                let sftp_channel = handle
                                    .channel_open_session()
                                    .await
                                    .map_err(|error| error.to_string())?;
                                sftp_channel
                                    .request_subsystem(true, "sftp")
                                    .await
                                    .map_err(|error| error.to_string())?;
                                SftpSession::new(sftp_channel.into_stream())
                                    .await
                                    .map_err(|error| error.to_string())
                            }
                            .await;
                            let _ = reply.send(result);
                        }
                        Some(SessionCommand::OpenExec { command, reply }) => {
                            let result = async {
                                let exec_channel = handle
                                    .channel_open_session()
                                    .await
                                    .map_err(|error| error.to_string())?;
                                exec_channel
                                    .exec(true, command)
                                    .await
                                    .map_err(|error| error.to_string())?;
                                Ok(exec_channel)
                            }
                            .await;
                            let _ = reply.send(result);
                        }
                        Some(SessionCommand::Disconnect) | None => {
                            let _ = channel.eof().await;
                            break;
                        }
                    }
                }
                message = channel.wait() => {
                    match message {
                        Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                            let _ = app_for_task.emit("ssh-event", SshEvent::output(&session_id_for_task, &data));
                        }
                        Some(ChannelMsg::ExitStatus { exit_status }) => {
                            let event = SshEvent {
                                session_id: session_id_for_task.clone(),
                                kind: "exit",
                                data_base64: None,
                                message: None,
                                exit_status: Some(exit_status),
                            };
                            let _ = app_for_task.emit("ssh-event", event);
                            break;
                        }
                        Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                        _ => {}
                    }
                }
            }
        }

        let _ = handle
            .disconnect(Disconnect::ByApplication, "LiteShell session closed", "")
            .await;
        sessions.write().await.remove(&session_id_for_task);
        let _ = app_for_task.emit(
            "ssh-event",
            SshEvent::state(&session_id_for_task, "disconnected"),
        );
    });

    Ok(ConnectOutcome::Connected {
        session_id: request.session_id,
        host: request.host,
        port: request.port,
        username: request.username,
    })
}

#[tauri::command]
pub async fn ssh_send(
    manager: State<'_, SessionManager>,
    session_id: String,
    data: String,
) -> Result<(), CommandError> {
    send_command(
        &manager,
        &session_id,
        SessionCommand::Input(data.into_bytes()),
    )
    .await
}

#[tauri::command]
pub async fn ssh_resize(
    manager: State<'_, SessionManager>,
    session_id: String,
    cols: u32,
    rows: u32,
) -> Result<(), CommandError> {
    send_command(
        &manager,
        &session_id,
        SessionCommand::Resize {
            cols: cols.clamp(20, 500),
            rows: rows.clamp(5, 300),
        },
    )
    .await
}

#[tauri::command]
pub async fn ssh_disconnect(
    manager: State<'_, SessionManager>,
    session_id: String,
) -> Result<(), CommandError> {
    send_command(&manager, &session_id, SessionCommand::Disconnect).await
}

async fn send_command(
    manager: &SessionManager,
    session_id: &str,
    command: SessionCommand,
) -> Result<(), CommandError> {
    let sender = manager
        .sessions
        .read()
        .await
        .get(session_id)
        .map(|control| control.commands.clone())
        .ok_or_else(|| CommandError::new("SESSION_NOT_FOUND", "SSH 会话不存在或已经断开"))?;
    sender
        .send(command)
        .await
        .map_err(|_| CommandError::new("SESSION_CLOSED", "SSH 会话已经关闭"))
}

pub(crate) async fn session_server_id(
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
    manager
        .sessions
        .read()
        .await
        .iter()
        .find_map(|(session_id, control)| {
            (control.identity.server_id() == server_id).then(|| session_id.clone())
        })
}

pub(crate) async fn open_sftp(
    manager: &SessionManager,
    session_id: &str,
) -> Result<SftpSession, CommandError> {
    let sender = manager
        .sessions
        .read()
        .await
        .get(session_id)
        .map(|control| control.commands.clone())
        .ok_or_else(|| CommandError::new("SESSION_NOT_FOUND", "SSH 会话不存在或已经断开"))?;
    let (reply, receiver) = oneshot::channel();
    sender
        .send(SessionCommand::OpenSftp(reply))
        .await
        .map_err(|_| CommandError::new("SESSION_CLOSED", "SSH 会话已经关闭"))?;
    receiver
        .await
        .map_err(|_| CommandError::new("SFTP_OPEN_FAILED", "SFTP 通道响应中断"))?
        .map_err(|error| CommandError::new("SFTP_OPEN_FAILED", error))
}

pub(crate) async fn exec_capture(
    manager: &SessionManager,
    session_id: &str,
    command: &'static str,
) -> Result<Vec<u8>, CommandError> {
    let sender = manager
        .sessions
        .read()
        .await
        .get(session_id)
        .map(|control| control.commands.clone())
        .ok_or_else(|| CommandError::new("SESSION_NOT_FOUND", "SSH 会话不存在或已经断开"))?;
    let (reply, receiver) = oneshot::channel();
    sender
        .send(SessionCommand::OpenExec {
            command: command.to_owned(),
            reply,
        })
        .await
        .map_err(|_| CommandError::new("SESSION_CLOSED", "SSH 会话已经关闭"))?;
    let mut channel = receiver
        .await
        .map_err(|_| CommandError::new("EXEC_OPEN_FAILED", "远程命令通道响应中断"))?
        .map_err(|error| CommandError::new("EXEC_OPEN_FAILED", error))?;
    let mut output = Vec::with_capacity(4096);
    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                if output.len() + data.len() > 1024 * 1024 {
                    return Err(CommandError::new(
                        "EXEC_OUTPUT_LIMIT",
                        "远程命令输出超过 1 MB",
                    ));
                }
                output.extend_from_slice(&data);
            }
            ChannelMsg::ExitStatus { exit_status } if exit_status != 0 => {
                return Err(CommandError::new(
                    "EXEC_FAILED",
                    format!("远程采样命令退出码：{exit_status}"),
                ));
            }
            ChannelMsg::Eof | ChannelMsg::Close => break,
            _ => {}
        }
    }
    Ok(output)
}

async fn authenticate(
    handle: &mut client::Handle<HostKeyHandler>,
    username: &str,
    auth: &AuthMethod,
) -> Result<(), CommandError> {
    let result = match auth {
        AuthMethod::Password { password } => handle
            .authenticate_password(username, password)
            .await
            .map_err(|error| CommandError::new("AUTH_FAILED", error.to_string()))?,
        AuthMethod::PrivateKey { path, passphrase } => {
            let key = load_secret_key(path, passphrase.as_deref())
                .map_err(|error| CommandError::new("PRIVATE_KEY_INVALID", error.to_string()))?;
            let hash = handle
                .best_supported_rsa_hash()
                .await
                .map_err(|error| CommandError::new("AUTH_FAILED", error.to_string()))?
                .flatten();
            handle
                .authenticate_publickey(username, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
                .await
                .map_err(|error| CommandError::new("AUTH_FAILED", error.to_string()))?
        }
    };

    if result.success() {
        Ok(())
    } else {
        Err(CommandError::new(
            "AUTH_REJECTED",
            "服务器拒绝了提供的认证信息",
        ))
    }
}

fn validate_request(request: &ConnectRequest) -> Result<(), CommandError> {
    if request.session_id.trim().is_empty() {
        return Err(CommandError::new("INVALID_SESSION_ID", "会话标识不能为空"));
    }
    if request.host.trim().is_empty() || request.username.trim().is_empty() {
        return Err(CommandError::new(
            "INVALID_CONNECTION",
            "主机地址和用户名不能为空",
        ));
    }
    if request.port == 0 {
        return Err(CommandError::new("INVALID_PORT", "端口必须大于 0"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(host: &str, username: &str, port: u16) -> ConnectRequest {
        ConnectRequest {
            session_id: "session-1".into(),
            host: host.into(),
            port,
            username: username.into(),
            auth: AuthMethod::Password {
                password: "secret".into(),
            },
            cols: 120,
            rows: 36,
            expected_host_fingerprint: None,
        }
    }

    #[test]
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
