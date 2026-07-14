from pathlib import Path


path = Path("scripts/apply_pr7.py")
text = path.read_text(encoding="utf-8")


def replace_once(old: str, new: str, label: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    text = text.replace(old, new, 1)


replace_once(
    '''use sftp_directory::{
    sftp_finish_directory_replacement, sftp_inspect_local_path, sftp_prepare_local_directory,
    sftp_prepare_remote_directory, DirectoryReplacementManager,
};
''',
    '''use sftp_directory::{
    sftp_finish_directory_replacement, sftp_inspect_local_path, sftp_inspect_remote_path,
    sftp_prepare_local_directory, sftp_prepare_remote_directory, DirectoryReplacementManager,
};
''',
    "remote inspection import",
)
replace_once(
    'export type LocalPathKind = "missing" | "file" | "directory" | "other";',
    'export type LocalPathKind = "missing" | "file" | "directory" | "symlink" | "other";',
    "remote symlink type",
)

path.write_text(text, encoding="utf-8", newline="\n")
Path("scripts/pre_fix_apply_pr7.py").unlink()
