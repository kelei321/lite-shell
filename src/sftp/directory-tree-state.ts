export type SftpDirectoryTreeEntry = {
  name: string;
  path: string;
  permissions: string;
};

export type SftpDirectoryTreeNode = {
  path: string;
  name: string;
  parentPath: string | null;
  permissions: string;
  children: string[];
  loaded: boolean;
  loading: boolean;
  expanded: boolean;
  error: string;
  requestVersion: number;
};

export type SftpDirectoryTreeState = {
  sessionId: string;
  rootPath: string;
  selectedPath: string;
  nodes: Map<string, SftpDirectoryTreeNode>;
};

export type VisibleDirectoryTreeNode = {
  node: SftpDirectoryTreeNode;
  depth: number;
};

function directoryName(path: string): string {
  if (path === "/") return "/";
  if (path === ".") return ".";
  const normalized = path.replace(/\/+$/, "");
  return normalized.slice(normalized.lastIndexOf("/") + 1) || normalized;
}

export function normalizeDirectoryTreePath(path: string): string {
  const trimmed = path.trim();
  if (!trimmed || trimmed === ".") return ".";
  const absolute = trimmed.startsWith("/");
  const parts: string[] = [];
  for (const part of trimmed.split("/")) {
    if (!part || part === ".") continue;
    if (part === "..") parts.pop();
    else parts.push(part);
  }
  if (absolute) return parts.length ? `/${parts.join("/")}` : "/";
  return parts.length ? parts.join("/") : ".";
}

export function directoryTreeAncestorPaths(path: string): string[] {
  const normalized = normalizeDirectoryTreePath(path);
  if (normalized === "/" || normalized === ".") return [normalized];
  const absolute = normalized.startsWith("/");
  const parts = normalized.split("/").filter(Boolean);
  const result: string[] = absolute ? ["/"] : ["."];
  let current = absolute ? "" : ".";
  for (const part of parts) {
    current = current === "." ? part : `${current}/${part}`;
    result.push(current || "/");
  }
  return result;
}

export function createSftpDirectoryTreeState(
  sessionId: string,
  rootPath = "/",
): SftpDirectoryTreeState {
  const normalizedRoot = normalizeDirectoryTreePath(rootPath);
  const rootNode: SftpDirectoryTreeNode = {
    path: normalizedRoot,
    name: directoryName(normalizedRoot),
    parentPath: null,
    permissions: "",
    children: [],
    loaded: false,
    loading: false,
    expanded: true,
    error: "",
    requestVersion: 0,
  };
  return {
    sessionId,
    rootPath: normalizedRoot,
    selectedPath: normalizedRoot,
    nodes: new Map([[normalizedRoot, rootNode]]),
  };
}

export function ensureSftpDirectoryTreeState(
  states: Map<string, SftpDirectoryTreeState>,
  sessionId: string,
): SftpDirectoryTreeState {
  const existing = states.get(sessionId);
  if (existing) return existing;
  const created = createSftpDirectoryTreeState(sessionId);
  states.set(sessionId, created);
  return created;
}

export function ensureDirectoryTreeNode(
  state: SftpDirectoryTreeState,
  path: string,
  parentPath: string | null = null,
  name = directoryName(path),
  permissions = "",
): SftpDirectoryTreeNode {
  const normalizedPath = normalizeDirectoryTreePath(path);
  const existing = state.nodes.get(normalizedPath);
  if (existing) {
    if (parentPath !== null) existing.parentPath = normalizeDirectoryTreePath(parentPath);
    if (name) existing.name = name;
    if (permissions) existing.permissions = permissions;
    return existing;
  }
  const created: SftpDirectoryTreeNode = {
    path: normalizedPath,
    name,
    parentPath: parentPath === null ? null : normalizeDirectoryTreePath(parentPath),
    permissions,
    children: [],
    loaded: false,
    loading: false,
    expanded: false,
    error: "",
    requestVersion: 0,
  };
  state.nodes.set(normalizedPath, created);
  return created;
}

export function beginDirectoryTreeRequest(
  state: SftpDirectoryTreeState,
  path: string,
): number {
  const node = ensureDirectoryTreeNode(state, path);
  node.requestVersion += 1;
  node.loading = true;
  node.error = "";
  return node.requestVersion;
}

export function isCurrentDirectoryTreeRequest(
  state: SftpDirectoryTreeState,
  path: string,
  requestVersion: number,
): boolean {
  return state.nodes.get(normalizeDirectoryTreePath(path))?.requestVersion === requestVersion;
}

export function applyDirectoryTreeListing(
  state: SftpDirectoryTreeState,
  requestedPath: string,
  requestVersion: number,
  canonicalPath: string,
  directories: SftpDirectoryTreeEntry[],
): boolean {
  const normalizedRequested = normalizeDirectoryTreePath(requestedPath);
  if (!isCurrentDirectoryTreeRequest(state, normalizedRequested, requestVersion)) return false;

  const normalizedCanonical = normalizeDirectoryTreePath(canonicalPath);
  const requestedNode = ensureDirectoryTreeNode(state, normalizedRequested);
  const parent = normalizedCanonical === normalizedRequested
    ? requestedNode
    : ensureDirectoryTreeNode(state, normalizedCanonical, requestedNode.parentPath);

  parent.children = directories.map((directory) => {
    const child = ensureDirectoryTreeNode(
      state,
      directory.path,
      normalizedCanonical,
      directory.name,
      directory.permissions,
    );
    return child.path;
  });
  parent.loaded = true;
  parent.loading = false;
  parent.error = "";
  if (requestedNode !== parent) {
    requestedNode.loading = false;
    requestedNode.loaded = true;
  }
  return true;
}

export function failDirectoryTreeRequest(
  state: SftpDirectoryTreeState,
  path: string,
  requestVersion: number,
  message: string,
): boolean {
  if (!isCurrentDirectoryTreeRequest(state, path, requestVersion)) return false;
  const node = ensureDirectoryTreeNode(state, path);
  node.loading = false;
  node.error = message;
  return true;
}

export function finishDirectoryTreeRequest(
  state: SftpDirectoryTreeState,
  path: string,
  requestVersion: number,
): boolean {
  if (!isCurrentDirectoryTreeRequest(state, path, requestVersion)) return false;
  ensureDirectoryTreeNode(state, path).loading = false;
  return true;
}

export function selectDirectoryTreePath(
  state: SftpDirectoryTreeState,
  path: string,
): string[] {
  const ancestors = directoryTreeAncestorPaths(path);
  state.selectedPath = normalizeDirectoryTreePath(path);
  for (let index = 0; index < ancestors.length; index += 1) {
    const current = ancestors[index];
    const parent = index > 0 ? ancestors[index - 1] : null;
    const node = ensureDirectoryTreeNode(state, current, parent);
    if (parent !== null) {
      const parentNode = ensureDirectoryTreeNode(
        state,
        parent,
        index > 1 ? ancestors[index - 2] : null,
      );
      if (!parentNode.children.includes(node.path)) parentNode.children.push(node.path);
    }
    if (index < ancestors.length - 1) node.expanded = true;
  }
  return ancestors;
}

export function visibleDirectoryTreeNodes(
  state: SftpDirectoryTreeState,
): VisibleDirectoryTreeNode[] {
  const result: VisibleDirectoryTreeNode[] = [];
  const visited = new Set<string>();
  const visit = (path: string, depth: number) => {
    if (visited.has(path)) return;
    visited.add(path);
    const node = state.nodes.get(path);
    if (!node) return;
    result.push({ node, depth });
    if (!node.expanded) return;
    for (const childPath of node.children) visit(childPath, depth + 1);
  };
  visit(state.rootPath, 0);
  return result;
}

export function removeSftpDirectoryTreeState(
  states: Map<string, SftpDirectoryTreeState>,
  sessionId: string,
): void {
  const state = states.get(sessionId);
  if (state) {
    for (const node of state.nodes.values()) node.requestVersion += 1;
  }
  states.delete(sessionId);
}
