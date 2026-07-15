from pathlib import Path

path = Path("src-tauri/src/sftp_queue.rs")
text = path.read_text(encoding="utf-8")

old_impl = '''impl QueueDirection {
    fn as_str(self) -> &'static str {
        match self {
            Self::Upload => "upload",
            Self::Download => "download",
        }
    }
}

'''
if text.count(old_impl) != 1:
    raise RuntimeError("QueueDirection::as_str block not found exactly once")
text = text.replace(old_impl, "", 1)

old_progress = '''async fn progress_loop(app: AppHandle) {
    let transfers = app.state::<SftpTransferManager>();
    let mut receiver = transfers.subscribe();
    drop(transfers);
    loop {
'''
new_progress = '''async fn progress_loop(app: AppHandle) {
    let mut receiver = app.state::<SftpTransferManager>().subscribe();
    loop {
'''
if text.count(old_progress) != 1:
    raise RuntimeError("progress loop block not found exactly once")
text = text.replace(old_progress, new_progress, 1)

path.write_text(text, encoding="utf-8")
