mod monitor;
mod profiles;
mod sftp;
mod sftp_recursive;
mod ssh;

use monitor::{system_metrics, SystemMonitor};
use profiles::{
    connection_manager_snapshot, connections_export, connections_import_apply,
    connections_import_preview, folder_delete, folder_save, profile_delete, profile_duplicate,
    profile_save, profiles_batch, profiles_list,
};
use sftp::{
    sftp_cancel_transfer, sftp_create_directory, sftp_delete, sftp_delete_recursive,
    sftp_delete_transfer_checkpoint, sftp_discard_transfer_checkpoint, sftp_download, sftp_list,
    sftp_list_transfer_checkpoints, sftp_prepare_local_directory, sftp_rename, sftp_upload,
    SftpTransferManager,
};
use sftp_recursive::{sftp_local_directory_manifest, sftp_remote_directory_manifest};
use ssh::{ssh_connect, ssh_connect_profile, ssh_disconnect, ssh_resize, ssh_send, SessionManager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(SessionManager::default())
        .manage(SystemMonitor::default())
        .manage(SftpTransferManager::default())
        .invoke_handler(tauri::generate_handler![
            ssh_connect,
            ssh_connect_profile,
            ssh_send,
            ssh_resize,
            ssh_disconnect,
            profiles_list,
            profile_save,
            profile_delete,
            profile_duplicate,
            connection_manager_snapshot,
            folder_save,
            folder_delete,
            profiles_batch,
            connections_export,
            connections_import_preview,
            connections_import_apply,
            sftp_list,
            sftp_upload,
            sftp_download,
            sftp_cancel_transfer,
            sftp_list_transfer_checkpoints,
            sftp_delete_transfer_checkpoint,
            sftp_discard_transfer_checkpoint,
            sftp_local_directory_manifest,
            sftp_remote_directory_manifest,
            sftp_prepare_local_directory,
            sftp_create_directory,
            sftp_rename,
            sftp_delete,
            sftp_delete_recursive,
            system_metrics
        ])
        .run(tauri::generate_context!())
        .expect("failed to run LiteShell");
}
