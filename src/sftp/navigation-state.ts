import type { SessionSftpEntry } from "./session-state";

export type RemoteBreadcrumb = {
  label: string;
  path: string;
};

export type SelectionUpdate = {
  paths: string[];
  anchorPath: string;
};

export type SelectionModifiers = {
  toggle: boolean;
  range: boolean;
  additiveRange: boolean;
};

export type Point = { x: number; y: number };
export type Rect = { left: number; top: number; right: number; bottom: number };

export function normalizeRemotePath(path: string): string {
  const trimmed = path.trim();
  if (!trimmed || trimmed === ".") return ".";

  const absolute = trimmed.startsWith("/");
  const stack: string[] = [];
  for (const part of trimmed.split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") stack.pop();
    else stack.push(part);
  }

  if (absolute) return stack.length ? `/${stack.join("/")}` : "/";
  return stack.length ? stack.join("/") : ".";
}

export function buildRemoteBreadcrumbs(path: string): RemoteBreadcrumb[] {
  const normalized = normalizeRemotePath(path);
  if (normalized === ".") return [{ label: ".", path: "." }];
  if (normalized === "/") return [{ label: "/", path: "/" }];

  const parts = normalized.split("/").filter(Boolean);
  const breadcrumbs: RemoteBreadcrumb[] = normalized.startsWith("/")
    ? [{ label: "/", path: "/" }]
    : [];
  let current = normalized.startsWith("/") ? "" : ".";
  for (const part of parts) {
    current = current === "." ? part : `${current}/${part}`;
    breadcrumbs.push({ label: part, path: current || "/" });
  }
  return breadcrumbs;
}

export function reconcileSelection(
  entries: SessionSftpEntry[],
  selectedEntries: SessionSftpEntry[],
): SessionSftpEntry[] {
  const selectedPaths = new Set(selectedEntries.map((entry) => entry.path));
  return entries.filter((entry) => selectedPaths.has(entry.path));
}

export function updateSelectionPaths(
  orderedPaths: string[],
  selectedPaths: string[],
  clickedPath: string,
  anchorPath: string | undefined,
  modifiers: SelectionModifiers,
): SelectionUpdate {
  if (modifiers.range && anchorPath) {
    const anchorIndex = orderedPaths.indexOf(anchorPath);
    const clickedIndex = orderedPaths.indexOf(clickedPath);
    if (anchorIndex >= 0 && clickedIndex >= 0) {
      const start = Math.min(anchorIndex, clickedIndex);
      const end = Math.max(anchorIndex, clickedIndex);
      const rangePaths = orderedPaths.slice(start, end + 1);
      return {
        paths: modifiers.additiveRange
          ? [...new Set([...selectedPaths, ...rangePaths])]
          : rangePaths,
        anchorPath,
      };
    }
  }

  if (modifiers.toggle) {
    const selected = new Set(selectedPaths);
    if (selected.has(clickedPath)) selected.delete(clickedPath);
    else selected.add(clickedPath);
    return { paths: orderedPaths.filter((path) => selected.has(path)), anchorPath: clickedPath };
  }

  return { paths: [clickedPath], anchorPath: clickedPath };
}

export function isPointInsideRect(point: Point, rect: Rect): boolean {
  return point.x >= rect.left
    && point.x <= rect.right
    && point.y >= rect.top
    && point.y <= rect.bottom;
}
