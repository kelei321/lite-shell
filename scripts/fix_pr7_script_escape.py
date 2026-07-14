from pathlib import Path

path = Path("scripts/apply_pr7_safety_fix.py")
text = path.read_text(encoding="utf-8")
old = r'''            "C:\\tmp\\file.txt.liteshell-task-1.part",'''
new = r'''            "C:\\\\tmp\\\\file.txt.liteshell-task-1.part",'''
count = text.count(old)
if count != 2:
    raise RuntimeError(f"expected two Windows path literals, found {count}")
path.write_text(text.replace(old, new), encoding="utf-8", newline="\n")
Path("scripts/fix_pr7_script_escape.py").unlink()
