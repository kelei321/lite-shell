from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if text.count(old) != 1:
        raise RuntimeError(f"{label}: expected one match, found {text.count(old)}")
    return text.replace(old, new, 1)


app_path = Path("src/App.vue")
app = app_path.read_text(encoding="utf-8")
app = replace_once(
    app,
    '''  const ancestors = selectDirectoryTreePath(treeState, path);
  for (const ancestor of ancestors.slice(0, -1)) {''',
    '''  const ancestors = selectDirectoryTreePath(treeState, path);
  const ancestorsToLoad = ancestors.length === 1 ? ancestors : ancestors.slice(0, -1);
  for (const ancestor of ancestorsToLoad) {''',
    "load selected root node",
)
app = replace_once(
    app,
    '''  if (node.expanded) {
    node.expanded = false;
    return;
  }
  node.expanded = true;
  if (!node.loaded || node.error) await loadDirectoryTreeNode(sessionId, path);''',
    '''  if (node.expanded && node.loaded && !node.error) {
    node.expanded = false;
    return;
  }
  node.expanded = true;
  if (!node.loaded || node.error) await loadDirectoryTreeNode(sessionId, path);''',
    "load expanded pending node",
)
app = replace_once(
    app,
    '''function refreshDirectoryTreeNode(path: string) {
  const sessionId = activeSessionId.value;
  if (sessionId) void loadDirectoryTreeNode(sessionId, path, true);
}''',
    '''function refreshDirectoryTreeNode(path: string) {
  const sessionId = activeSessionId.value;
  if (!sessionId) return;
  const treeState = ensureSftpDirectoryTreeState(directoryTreeStates, sessionId);
  ensureDirectoryTreeNode(treeState, path).expanded = true;
  void loadDirectoryTreeNode(sessionId, path, true);
}''',
    "expand refreshed node",
)
app_path.write_text(app, encoding="utf-8")

component_path = Path("src/components/sftp/SftpDirectoryTree.vue")
component = component_path.read_text(encoding="utf-8")
component = replace_once(
    component,
    ''':style="{ '--tree-depth': row.depth }"''',
    ''':style="{ paddingLeft: `${6 + row.depth * 16}px` }"''',
    "tree indentation style",
)
component_path.write_text(component, encoding="utf-8")

style_path = Path("src/styles.css")
style = style_path.read_text(encoding="utf-8")
style = replace_once(
    style,
    '''.sftp-directory-tree-row { min-width: max-content; height: 28px; display: grid; grid-template-columns: 19px minmax(110px, 1fr) 20px; align-items: center; padding-left: calc(6px + (var(--tree-depth) * 16px)); color: #aebcc3; }''',
    '''.sftp-directory-tree-row { min-width: max-content; height: 28px; display: grid; grid-template-columns: 19px minmax(110px, 1fr) 20px; align-items: center; color: #aebcc3; }''',
    "tree indentation CSS fallback",
)
style_path.write_text(style, encoding="utf-8")
