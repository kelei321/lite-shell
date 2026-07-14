from pathlib import Path


apply_path = Path("scripts/apply_pr7.py")
apply_text = apply_path.read_text(encoding="utf-8")


def replace_apply_once(old: str, new: str, label: str) -> None:
    global apply_text
    count = apply_text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    apply_text = apply_text.replace(old, new, 1)


replace_apply_once(
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
replace_apply_once(
    '''            sftp_remote_directory_manifest,
            sftp_inspect_local_path,
            sftp_prepare_local_directory,
''',
    '''            sftp_remote_directory_manifest,
            sftp_inspect_local_path,
            sftp_inspect_remote_path,
            sftp_prepare_local_directory,
''',
    "remote inspection command",
)
replace_apply_once(
    'export type LocalPathKind = "missing" | "file" | "directory" | "other";',
    'export type LocalPathKind = "missing" | "file" | "directory" | "symlink" | "other";',
    "remote symlink type",
)
apply_path.write_text(apply_text, encoding="utf-8", newline="\n")

fix_path = Path("scripts/fix_apply_pr7.py")
fix_text = fix_path.read_text(encoding="utf-8")
old_guard = '''    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)
'''
new_guard = '''    if count != 1:
        if label == "lib remote inspect command" and count == 0:
            return text
        raise RuntimeError(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)
'''
if fix_text.count(old_guard) != 1:
    raise RuntimeError("fix script guard not found")
fix_path.write_text(fix_text.replace(old_guard, new_guard, 1), encoding="utf-8", newline="\n")

Path("scripts/pre_fix_apply_pr7.py").unlink()
