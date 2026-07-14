from pathlib import Path


path = Path("scripts/apply_pr9_upload_review_fixes.py")
text = path.read_text(encoding="utf-8")
start_marker = "    text = replace_once(\n        text,\n        '''    if path.is_empty() || matches!(path, \"/\" | \".\" | \"..\") || path.contains("
end_marker = '        "remote path traversal validation",\n    )\n'
start = text.find(start_marker)
end = text.find(end_marker, start)
if start < 0 or end < 0:
    raise RuntimeError("remote path validation patch block not found")
text = text[:start] + text[end + len(end_marker):]
path.write_text(text, encoding="utf-8", newline="\n")
Path("scripts/fix_pr9_upload_review_script.py").unlink()
