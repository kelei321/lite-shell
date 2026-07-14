from pathlib import Path

path = Path("src-tauri/src/sftp.rs")
text = path.read_text(encoding="utf-8")
old = '''            manager.acquire_task("task-a").err().map(|error| error.code),
            "TRANSFER_TASK_BUSY"
'''
new = '''            manager.acquire_task("task-a").err().map(|error| error.code),
            Some("TRANSFER_TASK_BUSY")
'''
count = text.count(old)
if count != 1:
    raise RuntimeError(f"expected one task lock assertion, found {count}")
path.write_text(text.replace(old, new, 1), encoding="utf-8", newline="\n")

for temporary in (
    "scripts/apply_pr7_safety_fix.py",
    ".github/workflows/apply-pr7-option-assertion-fix.yml",
):
    candidate = Path(temporary)
    if candidate.exists():
        candidate.unlink()
