from pathlib import Path

path = Path("src-tauri/src/sftp.rs")
text = path.read_text(encoding="utf-8")
old = "        let mut invalid = checkpoint;"
new = "        let mut invalid = checkpoint.clone();"
count = text.count(old)
if count != 1:
    raise RuntimeError(f"expected one checkpoint move, found {count}")
path.write_text(text.replace(old, new, 1), encoding="utf-8", newline="\n")
Path("scripts/apply_pr7_safety_fix.py").unlink()
