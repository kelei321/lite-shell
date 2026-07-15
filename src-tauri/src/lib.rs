mod monitor;
mod profiles;
mod sftp;
mod sftp_directory;
mod sftp_queue;
mod sftp_recursive;
mod ssh;

use monitor::{system_metrics, SystemMonitor};
use profiles::{
    connection_manager_snapshot, connections_export, connections_import_apply,
    connections_import_preview, folder_delete, folder_save, profile_delete, profile_duplicate,
    profile_save, profiles_batch, profiles_list,
};
use sftp::{
    sftp_cancel_transfer, sftp_create_directory, sftp_delete, sftp_delete_recursive, sftp_list,
    sftp_rename, SftpTransferManager,
};
use sftp_directory::{
    sftp_finish_directory_replacement, sftp_inspect_local_path, sftp_inspect_remote_path,
    sftp_prepare_local_directory, sftp_prepare_remote_directory, DirectoryReplacementManager,
};
use sftp_queue::{
    initialize_transfer_queue, sftp_queue_cancel, sftp_queue_clear_completed, sftp_queue_enqueue,
    sftp_queue_list, sftp_queue_pause, sftp_queue_resume, sftp_queue_retry,
    sftp_queue_set_concurrency, sftp_queue_wake, SftpTransferQueue,
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
        .manage(SftpTransferQueue::default())
        .manage(DirectoryReplacementManager::default())
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                initialize_transfer_queue(handle).await;
            });
            Ok(())
        })
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
            sftp_cancel_transfer,
            sftp_queue_list,
            sftp_queue_enqueue,
            sftp_queue_pause,
            sftp_queue_resume,
            sftp_queue_retry,
            sftp_queue_cancel,
            sftp_queue_clear_completed,
            sftp_queue_set_concurrency,
            sftp_queue_wake,
            sftp_local_directory_manifest,
            sftp_remote_directory_manifest,
            sftp_inspect_local_path,
            sftp_inspect_remote_path,
            sftp_prepare_local_directory,
            sftp_prepare_remote_directory,
            sftp_finish_directory_replacement,
            sftp_create_directory,
            sftp_rename,
            sftp_delete,
            sftp_delete_recursive,
            system_metrics
        ])
        .run(tauri::generate_context!())
        .expect("failed to run LiteShell");
}
