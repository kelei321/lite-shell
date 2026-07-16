export type SftpPathShortcutEvent = {
  key: string;
  ctrlKey?: boolean;
  metaKey?: boolean;
  altKey?: boolean;
  shiftKey?: boolean;
};

export function isSftpPathShortcut(event: SftpPathShortcutEvent): boolean {
  if (event.altKey || event.shiftKey) return false;
  if (event.key === "F6") return true;
  return Boolean(event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "l";
}

export function registerSftpPathShortcuts(
  target: Window = window,
  root: ParentNode = document,
): () => void {
  const focusPathInput = () => {
    const input = root.querySelector<HTMLInputElement>('input[aria-label="编辑远程路径"]');
    if (!input) return false;
    input.focus();
    input.select();
    return true;
  };

  const handleKeyDown = (event: KeyboardEvent) => {
    if (!isSftpPathShortcut(event)) return;
    if (!root.querySelector(".file-browser")) return;

    const eventTarget = event.target instanceof Element ? event.target : null;
    if (eventTarget?.closest(".terminal-host, .dialog-backdrop")) return;

    event.preventDefault();
    event.stopPropagation();
    if (focusPathInput()) return;

    const editButton = root.querySelector<HTMLButtonElement>('button[aria-label="编辑远程路径"]');
    if (!editButton || editButton.disabled) return;
    editButton.click();
    target.requestAnimationFrame(() => focusPathInput());
  };

  target.addEventListener("keydown", handleKeyDown, true);
  return () => target.removeEventListener("keydown", handleKeyDown, true);
}
