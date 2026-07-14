from pathlib import Path

# Compatibility hook for the previously registered repair workflow.
Path("scripts/fix_pr7_script_escape.py").unlink()
