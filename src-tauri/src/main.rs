#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod monitor;
mod sftp;
mod ssh;

use sftp::SftpState;
use ssh::SshState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(SshState::default())
        .manage(SftpState::default())
        .invoke_handler(tauri::generate_handler![
            ssh::ssh_connect,
            ssh::ssh_write,
            ssh::ssh_resize,
            ssh::ssh_close,
            sftp::sftp_connect,
            sftp::sftp_list_dir,
            sftp::sftp_close,
            sftp::sftp_download_file,
            sftp::sftp_upload_file,
            sftp::sftp_mkdir,
            sftp::sftp_create_file,
            sftp::sftp_delete,
            sftp::sftp_delete_many,
            sftp::sftp_rename,
            sftp::sftp_paste,
            sftp::sftp_chmod,
            monitor::monitor_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run LiteShell");
}
