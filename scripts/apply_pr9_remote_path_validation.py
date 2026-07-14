from pathlib import Path


path = Path("src-tauri/src/sftp_directory.rs")
text = path.read_text(encoding="utf-8")
start_marker = '    if path.is_empty() || matches!(path, "/" | "." | "..") || path.contains('
start = text.find(start_marker, text.find("fn validate_remote_directory_path"))
end = text.find(" {\n", start)
if start < 0 or end < 0:
    raise RuntimeError("remote path validation condition not found")
validation = r'''    if path.is_empty()
        || matches!(path, "/" | "." | "..")
        || path.contains('\0')
        || path.contains('\\')
        || path.split('/').any(|component| component == "..")
    {
'''
text = text[:start] + validation + text[end + len(" {\n"):]
path.write_text(text, encoding="utf-8", newline="\n")
Path("scripts/apply_pr9_remote_path_validation.py").unlink()
