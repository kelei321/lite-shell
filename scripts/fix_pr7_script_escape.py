from pathlib import Path

path = Path("scripts/apply_pr7_safety_fix.py")
text = path.read_text(encoding="utf-8")

old_path = r'''            "C:\\tmp\\file.txt.liteshell-task-1.part",'''
new_path = r'''            "C:\\\\tmp\\\\file.txt.liteshell-task-1.part",'''
path_count = text.count(old_path)
if path_count != 2:
    raise RuntimeError(f"expected two Windows path literals, found {path_count}")
text = text.replace(old_path, new_path)

old_type_patch = r'''    service = replace_once(
        service,
        """  updatedAt: number;
};
""",
        """  updatedAt: number;
  availableSessionId?: string;
};
""",
        "checkpoint session type",
    )'''
new_type_patch = r'''    service = replace_once(
        service,
        """  transferred: number;
  createdAt: number;
  updatedAt: number;
};
""",
        """  transferred: number;
  createdAt: number;
  updatedAt: number;
  availableSessionId?: string;
};
""",
        "checkpoint session type",
    )'''
type_count = text.count(old_type_patch)
if type_count != 1:
    raise RuntimeError(f"expected one checkpoint type patch, found {type_count}")
text = text.replace(old_type_patch, new_type_patch)

path.write_text(text, encoding="utf-8", newline="\n")
Path("scripts/fix_pr7_script_escape.py").unlink()
