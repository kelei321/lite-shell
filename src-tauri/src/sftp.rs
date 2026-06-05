use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use ssh2::Session;
use std::{net::TcpStream, path::Path};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpListPayload {
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
    private_key_path: Option<String>,
    passphrase: Option<String>,
    path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileItem {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
}

#[tauri::command]
pub fn sftp_list(payload: SftpListPayload) -> Result<Vec<RemoteFileItem>, String> {
    inner_sftp_list(payload).map_err(|error| error.to_string())
}

fn inner_sftp_list(payload: SftpListPayload) -> Result<Vec<RemoteFileItem>> {
    let session = create_session(&payload)?;
    let sftp = session.sftp().context("create sftp failed")?;
    let entries = sftp
        .readdir(Path::new(&payload.path))
        .with_context(|| format!("read remote dir failed: {}", payload.path))?;

    let mut items = Vec::new();

    for (path, stat) in entries {
        let name = path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();

        if name == "." || name == ".." {
            continue;
        }

        let full_path = if payload.path.ends_with('/') {
            format!("{}{}", payload.path, name)
        } else {
            format!("{}/{}", payload.path, name)
        };

        items.push(RemoteFileItem {
            name,
            path: full_path,
            is_dir: stat.is_dir(),
            size: stat.size.unwrap_or(0),
        });
    }

    items.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
    });

    Ok(items)
}

fn create_session(payload: &SftpListPayload) -> Result<Session> {
    let tcp = TcpStream::connect((payload.host.as_str(), payload.port))
        .with_context(|| format!("connect {}:{} failed", payload.host, payload.port))?;

    let mut session = Session::new().context("create ssh session failed")?;
    session.set_tcp_stream(tcp);
    session.handshake().context("ssh handshake failed")?;

    match (&payload.password, &payload.private_key_path) {
        (Some(password), _) if !password.is_empty() => session
            .userauth_password(&payload.username, password)
            .context("password auth failed")?,
        (_, Some(private_key_path)) if !private_key_path.is_empty() => session
            .userauth_pubkey_file(
                &payload.username,
                None,
                Path::new(private_key_path),
                payload.passphrase.as_deref(),
            )
            .context("private key auth failed")?,
        _ => return Err(anyhow!("password or private key is required")),
    }

    if !session.authenticated() {
        return Err(anyhow!("ssh auth failed"));
    }

    Ok(session)
}
