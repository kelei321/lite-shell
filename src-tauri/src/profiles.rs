use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use keyring::{Entry, Error as KeyringError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::ssh::CommandError;

const CREDENTIAL_SERVICE: &str = "com.liteshell.desktop.ssh";
const STORE_VERSION: u8 = 2;
const DEFAULT_FOLDER_ID: &str = "folder-default";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: ProfileAuthType,
    pub private_key_path: Option<String>,
    #[serde(default = "default_group")]
    pub group: String,
    #[serde(default = "default_folder_id")]
    pub folder_id: String,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub has_secret: bool,
    #[serde(default)]
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionFolder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileAuthType {
    Password,
    PrivateKey,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProfileRequest {
    pub id: Option<String>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: ProfileAuthType,
    pub private_key_path: Option<String>,
    pub group: Option<String>,
    pub folder_id: Option<String>,
    pub favorite: Option<bool>,
    pub secret: Option<String>,
    pub remember_secret: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionManagerSnapshot {
    pub version: u8,
    pub folders: Vec<ConnectionFolder>,
    pub profiles: Vec<ConnectionProfile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveFolderRequest {
    pub id: Option<String>,
    pub name: String,
    pub parent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteFolderRequest {
    pub folder_id: String,
    pub strategy: FolderDeleteStrategy,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FolderDeleteStrategy {
    MoveToDefault,
    DeleteConnections,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchProfileRequest {
    pub profile_ids: Vec<String>,
    pub action: BatchProfileAction,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BatchProfileAction {
    Move { folder_id: String },
    Favorite { favorite: bool },
    Delete,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportSource {
    LiteShell,
    OpenSsh,
    FinalShell,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub source: ImportSource,
    pub path: String,
    pub new_count: usize,
    pub duplicate_count: usize,
    pub skipped_count: usize,
    pub folders: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: usize,
    pub duplicates: usize,
    pub skipped: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPackage {
    pub format: String,
    pub version: u8,
    pub exported_at: u64,
    pub folders: Vec<ConnectionFolder>,
    pub profiles: Vec<ExportProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProfile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: ProfileAuthType,
    pub private_key_path: Option<String>,
    pub folder_id: String,
    pub sort_order: i32,
    pub favorite: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProfileStore {
    #[serde(default = "legacy_version")]
    version: u8,
    #[serde(default)]
    folders: Vec<ConnectionFolder>,
    #[serde(default)]
    profiles: Vec<ConnectionProfile>,
}

pub struct ResolvedProfile {
    pub profile: ConnectionProfile,
    pub secret: Option<String>,
}

struct ImportCandidates {
    profiles: Vec<ExportProfile>,
    folders: Vec<ConnectionFolder>,
    skipped: usize,
    warnings: Vec<String>,
}

#[tauri::command]
pub async fn profiles_list(app: AppHandle) -> Result<Vec<ConnectionProfile>, CommandError> {
    Ok(manager_snapshot_inner(&profile_store_path(&app)?)?.profiles)
}

#[tauri::command]
pub async fn connection_manager_snapshot(
    app: AppHandle,
) -> Result<ConnectionManagerSnapshot, CommandError> {
    manager_snapshot_inner(&profile_store_path(&app)?)
}

#[tauri::command]
pub async fn profile_save(
    app: AppHandle,
    request: SaveProfileRequest,
) -> Result<ConnectionProfile, CommandError> {
    validate_profile(&request)?;
    save_profile(&profile_store_path(&app)?, request)
}

#[tauri::command]
pub async fn profile_delete(app: AppHandle, profile_id: String) -> Result<(), CommandError> {
    delete_profiles(&profile_store_path(&app)?, &[profile_id])
}

#[tauri::command]
pub async fn profile_duplicate(
    app: AppHandle,
    profile_id: String,
) -> Result<ConnectionProfile, CommandError> {
    let path = profile_store_path(&app)?;
    let mut store = load_store(&path)?;
    let source = store
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .cloned()
        .ok_or_else(|| CommandError::new("PROFILE_NOT_FOUND", "连接配置不存在"))?;
    let mut copy = source;
    copy.id = new_id("profile");
    copy.name = unique_profile_name(&store.profiles, &format!("{} 副本", copy.name));
    copy.has_secret = false;
    copy.updated_at = now_secs();
    copy.sort_order = next_profile_order(&store.profiles, &copy.folder_id);
    store.profiles.push(copy.clone());
    persist_store(&path, &store)?;
    Ok(copy)
}

#[tauri::command]
pub async fn folder_save(
    app: AppHandle,
    request: SaveFolderRequest,
) -> Result<ConnectionFolder, CommandError> {
    let path = profile_store_path(&app)?;
    let mut store = load_store(&path)?;
    let name = request.name.trim();
    if name.is_empty() {
        return Err(CommandError::new("INVALID_FOLDER", "文件夹名称不能为空"));
    }
    if let Some(parent) = request.parent_id.as_deref() {
        ensure_folder_exists(&store, parent)?;
    }
    let id = request.id.unwrap_or_else(|| new_id("folder"));
    if id == DEFAULT_FOLDER_ID {
        return Err(CommandError::new(
            "DEFAULT_FOLDER_LOCKED",
            "默认分组不能修改",
        ));
    }
    ensure_no_folder_cycle(&store, &id, request.parent_id.as_deref())?;
    let parent_id = request.parent_id;
    let mut folder = ConnectionFolder {
        id: id.clone(),
        name: name.to_owned(),
        parent_id: parent_id.clone(),
        sort_order: store
            .folders
            .iter()
            .filter(|folder| folder.parent_id == parent_id)
            .count() as i32,
    };
    if let Some(existing) = store.folders.iter_mut().find(|folder| folder.id == id) {
        folder.sort_order = existing.sort_order;
        *existing = folder.clone();
    } else {
        store.folders.push(folder.clone());
    }
    persist_store(&path, &store)?;
    Ok(folder)
}

#[tauri::command]
pub async fn folder_delete(
    app: AppHandle,
    request: DeleteFolderRequest,
) -> Result<(), CommandError> {
    if request.folder_id == DEFAULT_FOLDER_ID {
        return Err(CommandError::new(
            "DEFAULT_FOLDER_LOCKED",
            "默认分组不能删除",
        ));
    }
    let path = profile_store_path(&app)?;
    let mut store = load_store(&path)?;
    ensure_folder_exists(&store, &request.folder_id)?;
    let descendants = folder_descendants(&store, &request.folder_id);
    let affected = store
        .profiles
        .iter()
        .filter(|profile| descendants.contains(&profile.folder_id))
        .map(|profile| profile.id.clone())
        .collect::<Vec<_>>();
    match request.strategy {
        FolderDeleteStrategy::MoveToDefault => {
            for profile in &mut store.profiles {
                if descendants.contains(&profile.folder_id) {
                    profile.folder_id = DEFAULT_FOLDER_ID.to_owned();
                    profile.group = default_group();
                }
            }
        }
        FolderDeleteStrategy::DeleteConnections => {
            store
                .profiles
                .retain(|profile| !affected.contains(&profile.id));
        }
    }
    store
        .folders
        .retain(|folder| !descendants.contains(&folder.id));
    persist_store(&path, &store)?;
    if matches!(request.strategy, FolderDeleteStrategy::DeleteConnections) {
        for id in affected {
            delete_secret_if_present(&id)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn profiles_batch(
    app: AppHandle,
    request: BatchProfileRequest,
) -> Result<ConnectionManagerSnapshot, CommandError> {
    let path = profile_store_path(&app)?;
    let ids = request.profile_ids.into_iter().collect::<HashSet<_>>();
    if ids.is_empty() {
        return Err(CommandError::new("EMPTY_SELECTION", "未选择连接"));
    }
    let mut store = load_store(&path)?;
    let mut deleted = Vec::new();
    match request.action {
        BatchProfileAction::Move { folder_id } => {
            ensure_folder_exists(&store, &folder_id)?;
            for profile in &mut store.profiles {
                if ids.contains(&profile.id) {
                    profile.folder_id = folder_id.clone();
                    profile.updated_at = now_secs();
                }
            }
        }
        BatchProfileAction::Favorite { favorite } => {
            for profile in &mut store.profiles {
                if ids.contains(&profile.id) {
                    profile.favorite = favorite;
                    profile.updated_at = now_secs();
                }
            }
        }
        BatchProfileAction::Delete => {
            deleted = store
                .profiles
                .iter()
                .filter(|profile| ids.contains(&profile.id))
                .map(|profile| profile.id.clone())
                .collect();
            store.profiles.retain(|profile| !ids.contains(&profile.id));
        }
    }
    persist_store(&path, &store)?;
    for id in deleted {
        delete_secret_if_present(&id)?;
    }
    snapshot_from_store(store)
}

#[tauri::command]
pub async fn connections_export(app: AppHandle, path: String) -> Result<(), CommandError> {
    let store = load_store(&profile_store_path(&app)?)?;
    let package = ExportPackage {
        format: "liteshell-connections".to_owned(),
        version: STORE_VERSION,
        exported_at: now_secs(),
        folders: store.folders,
        profiles: store.profiles.iter().map(export_profile).collect(),
    };
    let data = serde_json::to_vec_pretty(&package)
        .map_err(|error| CommandError::new("EXPORT_FORMAT_FAILED", error.to_string()))?;
    fs::write(path, data)
        .map_err(|error| CommandError::new("EXPORT_WRITE_FAILED", error.to_string()))
}

#[tauri::command]
pub async fn connections_import_preview(
    app: AppHandle,
    source: ImportSource,
    path: String,
) -> Result<ImportPreview, CommandError> {
    let store = load_store(&profile_store_path(&app)?)?;
    let candidates = parse_import(source, Path::new(&path))?;
    let duplicates = candidates
        .profiles
        .iter()
        .filter(|candidate| is_duplicate(&store.profiles, candidate))
        .count();
    Ok(ImportPreview {
        source,
        path,
        new_count: candidates.profiles.len().saturating_sub(duplicates),
        duplicate_count: duplicates,
        skipped_count: candidates.skipped,
        folders: candidates
            .folders
            .iter()
            .map(|folder| folder.name.clone())
            .collect(),
        warnings: candidates.warnings,
    })
}

#[tauri::command]
pub async fn connections_import_apply(
    app: AppHandle,
    source: ImportSource,
    path: String,
) -> Result<ImportResult, CommandError> {
    let store_path = profile_store_path(&app)?;
    let mut store = load_store(&store_path)?;
    let candidates = parse_import(source, Path::new(&path))?;
    let mut folder_map: HashMap<String, String> = HashMap::new();
    let mut pending_folders = candidates.folders.clone();
    while !pending_folders.is_empty() {
        let before = pending_folders.len();
        pending_folders.retain(|folder| {
            let mapped_parent = match folder.parent_id.as_deref() {
                Some(parent) => match folder_map.get(parent) {
                    Some(mapped) => Some(mapped.clone()),
                    None => return true,
                },
                None => None,
            };
            let existing = store
                .folders
                .iter()
                .find(|current| current.name == folder.name && current.parent_id == mapped_parent)
                .cloned();
            let target = existing.unwrap_or_else(|| {
                let next = ConnectionFolder {
                    id: new_id("folder"),
                    name: folder.name.clone(),
                    parent_id: mapped_parent,
                    sort_order: folder.sort_order,
                };
                store.folders.push(next.clone());
                next
            });
            folder_map.insert(folder.id.clone(), target.id);
            false
        });
        if pending_folders.len() == before {
            for folder in pending_folders.drain(..) {
                let id = new_id("folder");
                store.folders.push(ConnectionFolder {
                    id: id.clone(),
                    name: folder.name,
                    parent_id: None,
                    sort_order: folder.sort_order,
                });
                folder_map.insert(folder.id, id);
            }
        }
    }
    let mut imported = 0;
    let mut duplicates = 0;
    for mut profile in candidates.profiles {
        if is_duplicate(&store.profiles, &profile) {
            duplicates += 1;
            continue;
        }
        profile.id = new_id("profile");
        profile.folder_id = folder_map
            .get(&profile.folder_id)
            .cloned()
            .unwrap_or_else(default_folder_id);
        let updated_at = now_secs();
        store.profiles.push(ConnectionProfile {
            id: profile.id,
            name: profile.name,
            host: profile.host,
            port: profile.port,
            username: profile.username,
            auth_type: profile.auth_type,
            private_key_path: profile.private_key_path,
            group: default_group(),
            folder_id: profile.folder_id,
            sort_order: profile.sort_order,
            favorite: profile.favorite,
            has_secret: false,
            updated_at,
        });
        imported += 1;
    }
    persist_store(&store_path, &store)?;
    Ok(ImportResult {
        imported,
        duplicates,
        skipped: candidates.skipped,
        warnings: candidates.warnings,
    })
}

pub async fn resolve_profile(
    app: &AppHandle,
    profile_id: String,
) -> Result<ResolvedProfile, CommandError> {
    let store = load_store(&profile_store_path(app)?)?;
    let profile = store
        .profiles
        .into_iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| CommandError::new("PROFILE_NOT_FOUND", "连接配置不存在"))?;
    let secret = if profile.has_secret {
        Some(read_secret(&profile.id)?)
    } else {
        None
    };
    Ok(ResolvedProfile { profile, secret })
}

fn profile_store_path(app: &AppHandle) -> Result<PathBuf, CommandError> {
    app.path()
        .app_data_dir()
        .map(|path| path.join("connections.json"))
        .map_err(|error| CommandError::new("PROFILE_PATH_FAILED", error.to_string()))
}

fn manager_snapshot_inner(path: &PathBuf) -> Result<ConnectionManagerSnapshot, CommandError> {
    snapshot_from_store(load_store(path)?)
}

fn snapshot_from_store(mut store: ProfileStore) -> Result<ConnectionManagerSnapshot, CommandError> {
    store
        .folders
        .sort_by_key(|folder| (folder.sort_order, folder.name.to_lowercase()));
    store.profiles.sort_by_key(|profile| {
        (
            !profile.favorite,
            profile.sort_order,
            profile.name.to_lowercase(),
        )
    });
    Ok(ConnectionManagerSnapshot {
        version: STORE_VERSION,
        folders: store.folders,
        profiles: store.profiles,
    })
}

fn validate_profile(request: &SaveProfileRequest) -> Result<(), CommandError> {
    if request.name.trim().is_empty()
        || request.host.trim().is_empty()
        || request.username.trim().is_empty()
        || request.port == 0
    {
        return Err(CommandError::new(
            "INVALID_PROFILE",
            "名称、主机、端口和用户名不能为空",
        ));
    }
    if request.auth_type == ProfileAuthType::PrivateKey
        && request
            .private_key_path
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
    {
        return Err(CommandError::new("INVALID_PROFILE", "私钥路径不能为空"));
    }
    Ok(())
}

fn load_store(path: &PathBuf) -> Result<ProfileStore, CommandError> {
    if !path.exists() {
        return Ok(empty_store());
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| CommandError::new("PROFILE_READ_FAILED", error.to_string()))?;
    let mut store: ProfileStore = serde_json::from_str(&contents)
        .map_err(|error| CommandError::new("PROFILE_FORMAT_INVALID", error.to_string()))?;
    if store.version < STORE_VERSION || store.folders.is_empty() {
        migrate_store(&mut store);
        persist_store(path, &store)?;
    }
    ensure_default_folder(&mut store);
    Ok(store)
}

fn migrate_store(store: &mut ProfileStore) {
    let mut groups = HashMap::new();
    ensure_default_folder(store);
    for profile in &mut store.profiles {
        let name = if profile.group.trim().is_empty() {
            default_group()
        } else {
            profile.group.clone()
        };
        if name == default_group() {
            profile.folder_id = default_folder_id();
            continue;
        }
        let folder_id = groups.entry(name.clone()).or_insert_with(|| {
            let id = new_id("folder");
            store.folders.push(ConnectionFolder {
                id: id.clone(),
                name,
                parent_id: None,
                sort_order: store.folders.len() as i32,
            });
            id
        });
        profile.folder_id = folder_id.clone();
    }
    store.version = STORE_VERSION;
}

fn persist_store(path: &PathBuf, store: &ProfileStore) -> Result<(), CommandError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| CommandError::new("PROFILE_WRITE_FAILED", error.to_string()))?;
    }
    let contents = serde_json::to_vec_pretty(store)
        .map_err(|error| CommandError::new("PROFILE_FORMAT_FAILED", error.to_string()))?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, contents)
        .map_err(|error| CommandError::new("PROFILE_WRITE_FAILED", error.to_string()))?;
    replace_file(&temp, path)
        .map_err(|error| CommandError::new("PROFILE_WRITE_FAILED", error.to_string()))
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::{ffi::OsStr, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let wide = |value: &OsStr| value.encode_wide().chain(Some(0)).collect::<Vec<_>>();
    let source = wide(source.as_os_str());
    let destination = wide(destination.as_os_str());
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

fn save_profile(
    path: &PathBuf,
    request: SaveProfileRequest,
) -> Result<ConnectionProfile, CommandError> {
    let mut store = load_store(path)?;
    let folder_id = request.folder_id.unwrap_or_else(default_folder_id);
    ensure_folder_exists(&store, &folder_id)?;
    let id = request.id.unwrap_or_else(|| new_id("profile"));
    let old = store.profiles.iter().find(|profile| profile.id == id);
    let has_secret = if request.remember_secret {
        if let Some(secret) = request
            .secret
            .as_deref()
            .filter(|secret| !secret.is_empty())
        {
            write_secret(&id, secret)?;
            true
        } else {
            old.map(|profile| profile.has_secret).unwrap_or(false)
        }
    } else {
        delete_secret_if_present(&id)?;
        false
    };
    let profile = ConnectionProfile {
        id: id.clone(),
        name: request.name.trim().to_owned(),
        host: request.host.trim().to_owned(),
        port: request.port,
        username: request.username.trim().to_owned(),
        auth_type: request.auth_type,
        private_key_path: request
            .private_key_path
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty()),
        group: request.group.unwrap_or_else(default_group),
        folder_id: folder_id.clone(),
        sort_order: old
            .map(|profile| profile.sort_order)
            .unwrap_or_else(|| next_profile_order(&store.profiles, &folder_id)),
        favorite: request.favorite.unwrap_or(false),
        has_secret,
        updated_at: now_secs(),
    };
    if let Some(existing) = store.profiles.iter_mut().find(|profile| profile.id == id) {
        *existing = profile.clone();
    } else {
        store.profiles.push(profile.clone());
    }
    persist_store(path, &store)?;
    Ok(profile)
}

fn delete_profiles(path: &PathBuf, ids: &[String]) -> Result<(), CommandError> {
    let mut store = load_store(path)?;
    let selected = ids.iter().collect::<HashSet<_>>();
    store
        .profiles
        .retain(|profile| !selected.contains(&profile.id));
    persist_store(path, &store)?;
    for id in ids {
        delete_secret_if_present(id)?;
    }
    Ok(())
}

fn parse_import(source: ImportSource, path: &Path) -> Result<ImportCandidates, CommandError> {
    match source {
        ImportSource::LiteShell => parse_liteshell(path),
        ImportSource::OpenSsh => parse_openssh(path),
        ImportSource::FinalShell => parse_finalshell(path),
    }
}

fn parse_liteshell(path: &Path) -> Result<ImportCandidates, CommandError> {
    let text = fs::read_to_string(path)
        .map_err(|error| CommandError::new("IMPORT_READ_FAILED", error.to_string()))?;
    let package: ExportPackage = serde_json::from_str(&text)
        .map_err(|error| CommandError::new("IMPORT_FORMAT_INVALID", error.to_string()))?;
    if package.format != "liteshell-connections" {
        return Err(CommandError::new(
            "IMPORT_FORMAT_INVALID",
            "不是 LiteShell 连接备份",
        ));
    }
    Ok(ImportCandidates {
        profiles: package.profiles,
        folders: package.folders,
        skipped: 0,
        warnings: Vec::new(),
    })
}

fn parse_openssh(path: &Path) -> Result<ImportCandidates, CommandError> {
    let text = fs::read_to_string(path)
        .map_err(|error| CommandError::new("IMPORT_READ_FAILED", error.to_string()))?;
    let folder = import_folder("OpenSSH");
    let mut profiles = Vec::new();
    let mut current: Option<ExportProfile> = None;
    let mut skipped = 0;
    let mut warnings = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let key = parts.next().unwrap_or_default().to_ascii_lowercase();
        let value = parts.next().unwrap_or_default().trim();
        if key == "host" {
            if let Some(profile) = current.take() {
                if !profile.host.is_empty() {
                    profiles.push(profile);
                }
            }
            let alias = value.split_whitespace().next().unwrap_or_default();
            if alias.contains('*') || alias.contains('?') || alias.starts_with('!') {
                skipped += 1;
                current = None;
            } else {
                current = Some(ExportProfile {
                    id: new_id("import"),
                    name: alias.to_owned(),
                    host: alias.to_owned(),
                    port: 22,
                    username: String::new(),
                    auth_type: ProfileAuthType::Password,
                    private_key_path: None,
                    folder_id: folder.id.clone(),
                    sort_order: profiles.len() as i32,
                    favorite: false,
                });
            }
        } else if let Some(profile) = current.as_mut() {
            match key.as_str() {
                "hostname" => profile.host = value.to_owned(),
                "user" => profile.username = value.to_owned(),
                "port" => profile.port = value.parse().unwrap_or(22),
                "identityfile" => {
                    profile.auth_type = ProfileAuthType::PrivateKey;
                    profile.private_key_path = Some(value.trim_matches('"').to_owned());
                }
                "proxyjump" => warnings.push(format!("{}：ProxyJump 未导入", profile.name)),
                _ => {}
            }
        }
    }
    if let Some(profile) = current {
        if !profile.host.is_empty() {
            profiles.push(profile);
        }
    }
    for profile in &mut profiles {
        if profile.username.is_empty() {
            profile.username = "root".to_owned();
        }
    }
    Ok(ImportCandidates {
        profiles,
        folders: vec![folder],
        skipped,
        warnings,
    })
}

fn parse_finalshell(path: &Path) -> Result<ImportCandidates, CommandError> {
    let folder = import_folder("FinalShell");
    let mut folders = vec![folder.clone()];
    let mut files = Vec::new();
    collect_finalshell_entries(path, &folder.id, &mut folders, &mut files)?;
    let mut profiles = Vec::new();
    let mut skipped = 0;
    let mut warnings = Vec::new();
    for (file, folder_id) in files {
        let text = match fs::read_to_string(&file) {
            Ok(text) => text,
            Err(error) => {
                skipped += 1;
                warnings.push(format!("{}：{}", file.display(), error));
                continue;
            }
        };
        let value: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(error) => {
                skipped += 1;
                warnings.push(format!("{}：JSON 无法解析（{}）", file.display(), error));
                continue;
            }
        };
        if let Some(profile) = finalshell_profile(&value, &folder_id, profiles.len() as i32) {
            profiles.push(profile);
        } else {
            skipped += 1;
        }
    }
    Ok(ImportCandidates {
        profiles,
        folders,
        skipped,
        warnings,
    })
}

fn finalshell_profile(value: &Value, folder_id: &str, sort_order: i32) -> Option<ExportProfile> {
    let object = value.as_object()?;
    let get = |keys: &[&str]| {
        keys.iter()
            .find_map(|key| object.get(*key))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let host = get(&["host", "hostname", "hostName", "ip"])?;
    let name = get(&["name", "connName", "title"]).unwrap_or_else(|| host.clone());
    let username = get(&["user_name", "username", "user"]).unwrap_or_else(|| "root".to_owned());
    let port = ["port", "sshPort"]
        .iter()
        .find_map(|key| object.get(*key))
        .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
        .unwrap_or(22) as u16;
    let private_key_path = get(&["privateKeyPath", "identityFile", "keyPath"]);
    Some(ExportProfile {
        id: new_id("import"),
        name,
        host,
        port,
        username,
        auth_type: if private_key_path.is_some() {
            ProfileAuthType::PrivateKey
        } else {
            ProfileAuthType::Password
        },
        private_key_path,
        folder_id: folder_id.to_owned(),
        sort_order,
        favorite: false,
    })
}

fn collect_finalshell_entries(
    path: &Path,
    folder_id: &str,
    folders: &mut Vec<ConnectionFolder>,
    files: &mut Vec<(PathBuf, String)>,
) -> Result<(), CommandError> {
    if path.is_file() {
        files.push((path.to_owned(), folder_id.to_owned()));
        return Ok(());
    }
    for entry in fs::read_dir(path)
        .map_err(|error| CommandError::new("IMPORT_READ_FAILED", error.to_string()))?
    {
        let entry =
            entry.map_err(|error| CommandError::new("IMPORT_READ_FAILED", error.to_string()))?;
        let path = entry.path();
        if path.is_dir() {
            let child_id = new_id("import-folder");
            folders.push(ConnectionFolder {
                id: child_id.clone(),
                name: entry.file_name().to_string_lossy().into_owned(),
                parent_id: Some(folder_id.to_owned()),
                sort_order: folders.len() as i32,
            });
            collect_finalshell_entries(&path, &child_id, folders, files)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some("json") {
            files.push((path, folder_id.to_owned()));
        }
    }
    Ok(())
}

fn export_profile(profile: &ConnectionProfile) -> ExportProfile {
    ExportProfile {
        id: profile.id.clone(),
        name: profile.name.clone(),
        host: profile.host.clone(),
        port: profile.port,
        username: profile.username.clone(),
        auth_type: profile.auth_type,
        private_key_path: profile.private_key_path.clone(),
        folder_id: profile.folder_id.clone(),
        sort_order: profile.sort_order,
        favorite: profile.favorite,
    }
}

fn is_duplicate(existing: &[ConnectionProfile], candidate: &ExportProfile) -> bool {
    existing.iter().any(|profile| {
        profile.host.eq_ignore_ascii_case(&candidate.host)
            && profile.port == candidate.port
            && profile.username.eq_ignore_ascii_case(&candidate.username)
    })
}

fn import_folder(name: &str) -> ConnectionFolder {
    ConnectionFolder {
        id: new_id("import-folder"),
        name: name.to_owned(),
        parent_id: None,
        sort_order: 0,
    }
}

fn ensure_default_folder(store: &mut ProfileStore) {
    if !store
        .folders
        .iter()
        .any(|folder| folder.id == DEFAULT_FOLDER_ID)
    {
        store.folders.insert(
            0,
            ConnectionFolder {
                id: default_folder_id(),
                name: default_group(),
                parent_id: None,
                sort_order: 0,
            },
        );
    }
}

fn ensure_folder_exists(store: &ProfileStore, id: &str) -> Result<(), CommandError> {
    if store.folders.iter().any(|folder| folder.id == id) {
        Ok(())
    } else {
        Err(CommandError::new("FOLDER_NOT_FOUND", "连接文件夹不存在"))
    }
}

fn ensure_no_folder_cycle(
    store: &ProfileStore,
    folder_id: &str,
    parent_id: Option<&str>,
) -> Result<(), CommandError> {
    let mut current = parent_id;
    let mut visited = HashSet::new();
    while let Some(id) = current {
        if id == folder_id || !visited.insert(id) {
            return Err(CommandError::new(
                "FOLDER_CYCLE",
                "文件夹不能移动到自身或子文件夹",
            ));
        }
        current = store
            .folders
            .iter()
            .find(|folder| folder.id == id)
            .and_then(|folder| folder.parent_id.as_deref());
    }
    Ok(())
}

fn folder_descendants(store: &ProfileStore, root: &str) -> HashSet<String> {
    let mut result = HashSet::from([root.to_owned()]);
    loop {
        let before = result.len();
        for folder in &store.folders {
            if folder
                .parent_id
                .as_ref()
                .is_some_and(|parent| result.contains(parent))
            {
                result.insert(folder.id.clone());
            }
        }
        if result.len() == before {
            return result;
        }
    }
}

fn unique_profile_name(profiles: &[ConnectionProfile], base: &str) -> String {
    if !profiles.iter().any(|profile| profile.name == base) {
        return base.to_owned();
    }
    (2..)
        .map(|index| format!("{base} {index}"))
        .find(|name| !profiles.iter().any(|profile| profile.name == *name))
        .unwrap()
}

fn next_profile_order(profiles: &[ConnectionProfile], folder_id: &str) -> i32 {
    profiles
        .iter()
        .filter(|profile| profile.folder_id == folder_id)
        .map(|profile| profile.sort_order)
        .max()
        .unwrap_or(-1)
        + 1
}

fn empty_store() -> ProfileStore {
    ProfileStore {
        version: STORE_VERSION,
        folders: vec![ConnectionFolder {
            id: default_folder_id(),
            name: default_group(),
            parent_id: None,
            sort_order: 0,
        }],
        profiles: Vec::new(),
    }
}

fn credential_entry(profile_id: &str) -> Result<Entry, CommandError> {
    Entry::new(CREDENTIAL_SERVICE, profile_id)
        .map_err(|error| CommandError::new("CREDENTIAL_FAILED", error.to_string()))
}

fn write_secret(profile_id: &str, secret: &str) -> Result<(), CommandError> {
    credential_entry(profile_id)?
        .set_password(secret)
        .map_err(|error| CommandError::new("CREDENTIAL_WRITE_FAILED", error.to_string()))
}

fn read_secret(profile_id: &str) -> Result<String, CommandError> {
    credential_entry(profile_id)?
        .get_password()
        .map_err(|error| CommandError::new("CREDENTIAL_READ_FAILED", error.to_string()))
}

fn delete_secret_if_present(profile_id: &str) -> Result<(), CommandError> {
    match credential_entry(profile_id)?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
        Err(error) => Err(CommandError::new(
            "CREDENTIAL_DELETE_FAILED",
            error.to_string(),
        )),
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn new_id(prefix: &str) -> String {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{prefix}-{value:x}")
}

fn legacy_version() -> u8 {
    1
}

fn default_group() -> String {
    "默认分组".to_owned()
}

fn default_folder_id() -> String {
    DEFAULT_FOLDER_ID.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_v1_groups_without_changing_profile_ids() {
        let mut store = ProfileStore {
            version: 1,
            folders: Vec::new(),
            profiles: vec![ConnectionProfile {
                id: "profile-existing".into(),
                name: "生产".into(),
                host: "10.0.0.1".into(),
                port: 22,
                username: "root".into(),
                auth_type: ProfileAuthType::Password,
                private_key_path: None,
                group: "生产环境".into(),
                folder_id: default_folder_id(),
                sort_order: 0,
                favorite: false,
                has_secret: true,
                updated_at: 1,
            }],
        };
        migrate_store(&mut store);
        assert_eq!(store.version, 2);
        assert_eq!(store.profiles[0].id, "profile-existing");
        assert_ne!(store.profiles[0].folder_id, DEFAULT_FOLDER_ID);
    }

    #[test]
    fn rejects_folder_cycles() {
        let mut store = empty_store();
        store.folders.push(ConnectionFolder {
            id: "a".into(),
            name: "A".into(),
            parent_id: None,
            sort_order: 1,
        });
        store.folders.push(ConnectionFolder {
            id: "b".into(),
            name: "B".into(),
            parent_id: Some("a".into()),
            sort_order: 0,
        });
        assert_eq!(
            ensure_no_folder_cycle(&store, "a", Some("b"))
                .unwrap_err()
                .code,
            "FOLDER_CYCLE"
        );
    }

    #[test]
    fn parses_openssh_and_skips_wildcards() {
        let path = std::env::temp_dir().join(format!("liteshell-ssh-{}", now_secs()));
        fs::write(&path, "Host prod\n HostName 10.0.0.2\n User deploy\n Port 2222\n IdentityFile ~/.ssh/id_ed25519\nHost *\n User root\n").unwrap();
        let result = parse_openssh(&path).unwrap();
        fs::remove_file(path).ok();
        assert_eq!(result.profiles.len(), 1);
        assert_eq!(result.profiles[0].port, 2222);
        assert_eq!(result.skipped, 1);
    }

    #[test]
    fn export_profile_contains_no_secret() {
        let profile = ConnectionProfile {
            id: "p".into(),
            name: "测试".into(),
            host: "localhost".into(),
            port: 22,
            username: "root".into(),
            auth_type: ProfileAuthType::Password,
            private_key_path: None,
            group: default_group(),
            folder_id: default_folder_id(),
            sort_order: 0,
            favorite: false,
            has_secret: true,
            updated_at: 1,
        };
        let json = serde_json::to_string(&export_profile(&profile)).unwrap();
        assert!(!json.contains("secret"));
        assert!(!json.contains("hasSecret"));
    }
}
