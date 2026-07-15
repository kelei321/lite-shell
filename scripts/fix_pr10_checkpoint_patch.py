from pathlib import Path

path = Path("scripts/apply_pr10_checkpoint_truth.py")
text = path.read_text(encoding="utf-8")
old = "service = service[:legacy_commands_start] + service[legacy_commands_end:]"
new = '''service = (
    service[:legacy_commands_start]
    + '''\'''\'''export const cancelSftpTransfer = (transferId: string) =>
  invoke<void>("sftp_cancel_transfer", { transferId });

'''\'''\'''
    + service[legacy_commands_end:]
)'''
if text.count(old) != 1:
    raise RuntimeError(f"legacy command removal pattern count: {text.count(old)}")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
