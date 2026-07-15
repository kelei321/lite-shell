from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if text.count(old) != 1:
        raise RuntimeError(f"{label}: expected one match, found {text.count(old)}")
    return text.replace(old, new, 1)


# Rust SFTP command.
sftp_path = Path("src-tauri/src/sftp.rs")
sftp = sftp_path.read_text(encoding="utf-8")
sftp = replace_once(
    sftp,
    '''#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListing {
    path: String,
    entries: Vec<SftpEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpEntry {''',
    '''#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryListing {
    path: String,
    entries: Vec<SftpEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryTreeListing {
    path: String,
    directories: Vec<SftpDirectoryTreeEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpDirectoryTreeEntry {
    name: String,
    path: String,
    permissions: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpEntry {''',
    "directory tree response structs",
)

sftp = replace_once(
    sftp,
    '''#[tauri::command]
pub async fn sftp_upload(''',
    '''#[tauri::command]
pub async fn sftp_list_directories(
    manager: State<'_, SessionManager>,
    session_id: String,
    path: String,
) -> Result<DirectoryTreeListing, CommandError> {
    let sftp = open_sftp(&manager, &session_id).await?;
    let result = async {
        let canonical_path = sftp
            .canonicalize(if path.trim().is_empty() { "." } else { &path })
            .await
            .map_err(sftp_error("SFTP_TREE_PATH_FAILED"))?;
        let directory = sftp
            .read_dir(canonical_path.clone())
            .await
            .map_err(sftp_error("SFTP_TREE_LIST_FAILED"))?;
        let mut directories = directory
            .filter_map(|entry| {
                let name = entry.file_name();
                let file_type = entry.file_type();
                if matches!(name.as_str(), "." | "..")
                    || !file_type.is_dir()
                    || file_type.is_symlink()
                {
                    return None;
                }
                let metadata = entry.metadata();
                Some(SftpDirectoryTreeEntry {
                    name,
                    path: entry.path(),
                    permissions: format!("d{}", metadata.permissions()),
                })
            })
            .collect::<Vec<_>>();
        directories.sort_by_key(|entry| entry.name.to_lowercase());
        Ok(DirectoryTreeListing {
            path: canonical_path,
            directories,
        })
    }
    .await;
    sftp.close().await.ok();
    result
}

#[tauri::command]
pub async fn sftp_upload(''',
    "directory tree command",
)
sftp_path.write_text(sftp, encoding="utf-8")

# Tauri command registration.
lib_path = Path("src-tauri/src/lib.rs")
lib = lib_path.read_text(encoding="utf-8")
lib = replace_once(
    lib,
    '''    sftp_cancel_transfer, sftp_create_directory, sftp_delete, sftp_delete_recursive, sftp_list,
    sftp_rename, SftpTransferManager,''',
    '''    sftp_cancel_transfer, sftp_create_directory, sftp_delete, sftp_delete_recursive, sftp_list,
    sftp_list_directories, sftp_rename, SftpTransferManager,''',
    "directory tree command import",
)
lib = replace_once(
    lib,
    '''            sftp_list,
            sftp_cancel_transfer,''',
    '''            sftp_list,
            sftp_list_directories,
            sftp_cancel_transfer,''',
    "directory tree command registration",
)
lib_path.write_text(lib, encoding="utf-8")

# Frontend API.
service_path = Path("src/services/ssh.ts")
service = service_path.read_text(encoding="utf-8")
service = replace_once(
    service,
    '''export type DirectoryListing = {
  path: string;
  entries: SftpEntry[];
};

export type RecursiveScanSummary = {''',
    '''export type DirectoryListing = {
  path: string;
  entries: SftpEntry[];
};

export type SftpDirectoryTreeEntry = {
  name: string;
  path: string;
  permissions: string;
};

export type DirectoryTreeListing = {
  path: string;
  directories: SftpDirectoryTreeEntry[];
};

export type RecursiveScanSummary = {''',
    "directory tree API types",
)
service = replace_once(
    service,
    '''export const listSftpDirectory = (sessionId: string, path: string) =>
  invoke<DirectoryListing>("sftp_list", { sessionId, path });

export const cancelSftpTransfer = (transferId: string) =>''',
    '''export const listSftpDirectory = (sessionId: string, path: string) =>
  invoke<DirectoryListing>("sftp_list", { sessionId, path });

export const listSftpDirectories = (sessionId: string, path: string) =>
  invoke<DirectoryTreeListing>("sftp_list_directories", { sessionId, path });

export const cancelSftpTransfer = (transferId: string) =>''',
    "directory tree API command",
)
service_path.write_text(service, encoding="utf-8")

# Main Vue integration.
app_path = Path("src/App.vue")
app = app_path.read_text(encoding="utf-8")
app = replace_once(
    app,
    '''import ConnectionManager from "./components/ConnectionManager.vue";
import {''',
    '''import ConnectionManager from "./components/ConnectionManager.vue";
import SftpDirectoryTree from "./components/sftp/SftpDirectoryTree.vue";
import {''',
    "directory tree component import",
)
app = replace_once(
    app,
    '''  listProfiles,
  listSftpDirectory,
  prepareLocalDirectory,''',
    '''  listProfiles,
  listSftpDirectories,
  listSftpDirectory,
  prepareLocalDirectory,''',
    "directory tree service import",
)
app = replace_once(
    app,
    '''import { useSftpTransferQueue } from "./sftp/transfer-queue";
import {''',
    '''import { useSftpTransferQueue } from "./sftp/transfer-queue";
import {
  applyDirectoryTreeListing,
  beginDirectoryTreeRequest,
  ensureDirectoryTreeNode,
  ensureSftpDirectoryTreeState,
  failDirectoryTreeRequest,
  finishDirectoryTreeRequest,
  removeSftpDirectoryTreeState,
  selectDirectoryTreePath,
  type SftpDirectoryTreeState,
} from "./sftp/directory-tree-state";
import {''',
    "directory tree state imports",
)
app = replace_once(
    app,
    '''  IconChevronDown,
  IconChevronLeft,
  IconChevronUp,''',
    '''  IconChevronDown,
  IconChevronLeft,
  IconChevronRight,
  IconChevronUp,''',
    "directory tree expand icon",
)
app = replace_once(
    app,
    '''const selectedTool = ref<"files" | "bookmarks" | "history">("files");
const sftpStates = reactive(new Map<string, SftpSessionState>());''',
    '''const selectedTool = ref<"files" | "bookmarks" | "history">("files");
const directoryTreeStates = reactive(new Map<string, SftpDirectoryTreeState>());
const emptyDirectoryTreeState = reactive(ensureSftpDirectoryTreeState(new Map(), ""));
const activeDirectoryTreeState = computed(() => activeSessionId.value
  ? ensureSftpDirectoryTreeState(directoryTreeStates, activeSessionId.value)
  : emptyDirectoryTreeState);
const storedSftpTreeWidth = Number(localStorage.getItem("liteshell.sftp.tree-width.v1"));
const sftpTreeWidth = ref(Number.isFinite(storedSftpTreeWidth)
  ? Math.min(420, Math.max(160, storedSftpTreeWidth))
  : 224);
const sftpTreeCollapsed = ref(false);
const sftpStates = reactive(new Map<string, SftpSessionState>());''',
    "directory tree state variables",
)
app = replace_once(
    app,
    '''let unlistenDragDrop: (() => void) | null = null;
let monitorTimer: number | null = null;''',
    '''let unlistenDragDrop: (() => void) | null = null;
let stopSftpTreeResize: (() => void) | null = null;
let monitorTimer: number | null = null;''',
    "directory tree resize cleanup",
)
app = replace_once(
    app,
    '''  terminalBuffers.delete(id);
  removeSftpSessionState(sftpStates, id);''',
    '''  terminalBuffers.delete(id);
  removeSftpSessionState(sftpStates, id);
  removeSftpDirectoryTreeState(directoryTreeStates, id);''',
    "directory tree session cleanup",
)

app = replace_once(
    app,
    '''async function loadDirectory(sessionId: string, path: string, recordHistory = true) {''',
    '''async function loadDirectoryTreeNode(sessionId: string, path: string, force = false) {
  const session = sessions.value.find((item) => item.id === sessionId);
  const treeState = ensureSftpDirectoryTreeState(directoryTreeStates, sessionId);
  const node = ensureDirectoryTreeNode(treeState, path);
  if (!session?.connected || !isTauri()) {
    node.loading = false;
    node.error = "请先建立 SSH 连接";
    return;
  }
  if (node.loaded && !force && !node.error) return;

  const requestVersion = beginDirectoryTreeRequest(treeState, node.path);
  try {
    const listing = await listSftpDirectories(sessionId, node.path);
    applyDirectoryTreeListing(
      treeState,
      node.path,
      requestVersion,
      listing.path,
      listing.directories,
    );
  } catch (error) {
    failDirectoryTreeRequest(
      treeState,
      node.path,
      requestVersion,
      describeCommandError(error),
    );
  } finally {
    finishDirectoryTreeRequest(treeState, node.path, requestVersion);
  }
}

async function synchronizeDirectoryTreePath(sessionId: string, path: string) {
  const treeState = ensureSftpDirectoryTreeState(directoryTreeStates, sessionId);
  const ancestors = selectDirectoryTreePath(treeState, path);
  for (const ancestor of ancestors.slice(0, -1)) {
    const node = ensureDirectoryTreeNode(treeState, ancestor);
    node.expanded = true;
    if (!node.loaded && !node.loading) await loadDirectoryTreeNode(sessionId, ancestor);
  }
}

async function toggleDirectoryTreeNode(path: string) {
  const sessionId = activeSessionId.value;
  if (!sessionId) return;
  const treeState = ensureSftpDirectoryTreeState(directoryTreeStates, sessionId);
  const node = ensureDirectoryTreeNode(treeState, path);
  if (node.expanded) {
    node.expanded = false;
    return;
  }
  node.expanded = true;
  if (!node.loaded || node.error) await loadDirectoryTreeNode(sessionId, path);
}

async function openDirectoryTreePath(path: string) {
  const sessionId = activeSessionId.value;
  if (!sessionId) return;
  selectedTool.value = "files";
  await loadDirectory(sessionId, path);
}

function refreshDirectoryTreeNode(path: string) {
  const sessionId = activeSessionId.value;
  if (sessionId) void loadDirectoryTreeNode(sessionId, path, true);
}

function beginSftpTreeResize(event: PointerEvent) {
  if (sftpTreeCollapsed.value) return;
  event.preventDefault();
  stopSftpTreeResize?.();
  const startX = event.clientX;
  const startWidth = sftpTreeWidth.value;
  const handleMove = (moveEvent: PointerEvent) => {
    sftpTreeWidth.value = Math.min(420, Math.max(160, startWidth + moveEvent.clientX - startX));
  };
  const handleStop = () => {
    window.removeEventListener("pointermove", handleMove);
    window.removeEventListener("pointerup", handleStop);
    localStorage.setItem("liteshell.sftp.tree-width.v1", String(sftpTreeWidth.value));
    stopSftpTreeResize = null;
  };
  stopSftpTreeResize = handleStop;
  window.addEventListener("pointermove", handleMove);
  window.addEventListener("pointerup", handleStop, { once: true });
}

async function loadDirectory(sessionId: string, path: string, recordHistory = true) {''',
    "directory tree functions",
)
app = replace_once(
    app,
    '''      writeSftpStorage(sessionId, "liteshell.sftp.history.v1", state.recentPaths);
    }
  } catch (error) {''',
    '''      writeSftpStorage(sessionId, "liteshell.sftp.history.v1", state.recentPaths);
    }
    void synchronizeDirectoryTreePath(sessionId, listing.path);
  } catch (error) {''',
    "directory tree path synchronization",
)
app = replace_once(
    app,
    '''  unlistenDragDrop?.();
  if (monitorTimer !== null) window.clearInterval(monitorTimer);''',
    '''  unlistenDragDrop?.();
  stopSftpTreeResize?.();
  if (monitorTimer !== null) window.clearInterval(monitorTimer);''',
    "directory tree resize teardown",
)

old_shell = '''      <section ref="sftpPaneElement" class="sftp-pane">
        <div v-if="sftpDragActive" class="sftp-drop-overlay"><IconUpload :size="38" /><strong>上传到 {{ sftpPath }}</strong><span>松开鼠标上传文件或文件夹</span></div>
        <aside class="sftp-tools">
          <strong>SFTP</strong>
          <button :class="{ active: selectedTool === 'files' }" @click="selectedTool = 'files'"><IconFolder :size="24" /><span>文件</span></button>
          <button :class="{ active: selectedTool === 'bookmarks' }" @click="selectedTool = 'bookmarks'"><IconStar :size="25" /><span>书签</span></button>
          <button :class="{ active: selectedTool === 'history' }" @click="selectedTool = 'history'"><IconClockHour4 :size="25" /><span>历史</span></button>
        </aside>
        <div class="file-browser">'''
new_shell = '''      <section
        ref="sftpPaneElement"
        class="sftp-pane"
        :style="{ '--sftp-tree-width': `${sftpTreeCollapsed ? 0 : sftpTreeWidth}px` }"
      >
        <div v-if="sftpDragActive" class="sftp-drop-overlay"><IconUpload :size="38" /><strong>上传到 {{ sftpPath }}</strong><span>松开鼠标上传文件或文件夹</span></div>
        <aside class="sftp-tree-pane" :class="{ collapsed: sftpTreeCollapsed }">
          <div class="sftp-tree-tabs">
            <button :class="{ active: selectedTool === 'files' }" @click="selectedTool = 'files'"><IconFolder :size="15" />目录</button>
            <button :class="{ active: selectedTool === 'bookmarks' }" @click="selectedTool = 'bookmarks'"><IconStar :size="15" />书签</button>
            <button :class="{ active: selectedTool === 'history' }" @click="selectedTool = 'history'"><IconClockHour4 :size="15" />历史</button>
            <button class="sftp-tree-collapse icon-button" aria-label="折叠目录树" @click="sftpTreeCollapsed = true"><IconChevronLeft :size="16" /></button>
          </div>
          <SftpDirectoryTree
            v-if="selectedTool === 'files'"
            :state="activeDirectoryTreeState"
            :connected="Boolean(activeSession?.connected)"
            @open="openDirectoryTreePath"
            @toggle="toggleDirectoryTreeNode"
            @refresh="refreshDirectoryTreeNode"
          />
          <div v-else-if="selectedTool === 'bookmarks'" class="sftp-tree-location-list">
            <header><strong>路径书签</strong><span>{{ sftpBookmarks.length }} 项</span></header>
            <button v-for="path in sftpBookmarks" :key="path" @dblclick="selectedTool = 'files'; loadActiveDirectory(path)"><IconBookmark :size="15" /><span>{{ path }}</span><IconX :size="14" @click.stop="removeSftpBookmark(path)" /></button>
            <div v-if="!sftpBookmarks.length" class="sftp-directory-tree-empty"><IconBookmark :size="28" /><strong>暂无书签</strong><span>在地址栏收藏常用目录</span></div>
          </div>
          <div v-else class="sftp-tree-location-list">
            <header><strong>访问历史</strong><button :disabled="!sftpRecentPaths.length" @click="clearSftpHistory">清空</button></header>
            <button v-for="path in sftpRecentPaths" :key="path" @dblclick="selectedTool = 'files'; loadActiveDirectory(path)"><IconClockHour4 :size="15" /><span>{{ path }}</span></button>
            <div v-if="!sftpRecentPaths.length" class="sftp-directory-tree-empty"><IconClockHour4 :size="28" /><strong>暂无历史</strong><span>浏览过的远程目录会显示在这里</span></div>
          </div>
        </aside>
        <div class="sftp-tree-resizer" role="separator" aria-orientation="vertical" @pointerdown="beginSftpTreeResize"></div>
        <button v-if="sftpTreeCollapsed" class="sftp-tree-expand icon-button" aria-label="展开目录树" @click="sftpTreeCollapsed = false"><IconChevronRight :size="17" /></button>
        <div class="file-browser">'''
app = replace_once(app, old_shell, new_shell, "SFTP dual-pane shell")
app = replace_once(
    app,
    '''          <div v-if="selectedTool === 'files'" class="file-table" role="table" aria-label="远程文件">''',
    '''          <div class="file-table" role="table" aria-label="远程文件">''',
    "always-visible file table",
)
for obsolete in [
    '''          <div v-else-if="selectedTool === 'bookmarks'" class="sftp-location-list"><header><strong>路径书签</strong><span>{{ sftpBookmarks.length }} 项</span></header><button v-for="path in sftpBookmarks" :key="path" @dblclick="selectedTool = 'files'; loadActiveDirectory(path)"><IconBookmark :size="17" /><span>{{ path }}</span><small>双击打开</small><IconX :size="15" @click.stop="removeSftpBookmark(path)" /></button><div v-if="!sftpBookmarks.length" class="empty-tool-state"><IconBookmark :size="34" /><strong>暂无书签</strong><span>在路径栏点击星标收藏常用目录</span></div></div>
''',
    '''          <div v-else class="sftp-location-list"><header><strong>访问历史</strong><button :disabled="!sftpRecentPaths.length" @click="clearSftpHistory">清空</button></header><button v-for="path in sftpRecentPaths" :key="path" @dblclick="selectedTool = 'files'; loadActiveDirectory(path)"><IconClockHour4 :size="17" /><span>{{ path }}</span><small>双击打开</small></button><div v-if="!sftpRecentPaths.length" class="empty-tool-state"><IconClockHour4 :size="34" /><strong>暂无历史记录</strong><span>浏览过的远程目录会显示在这里</span></div></div>
''',
]:
    if obsolete not in app:
        raise RuntimeError("obsolete right-side location panel not found")
    app = app.replace(obsolete, "", 1)
app_path.write_text(app, encoding="utf-8")

# Dual-pane and tree styles.
style_path = Path("src/styles.css")
style = style_path.read_text(encoding="utf-8")
style = replace_once(
    style,
    '''.sftp-pane { position: relative; min-height: 0; display: grid; grid-template-columns: 72px minmax(0, 1fr); background: var(--sftp); }
.sftp-tools { display: flex; flex-direction: column; border-right: 1px solid var(--line); }
.sftp-tools > strong { height: 48px; display: grid; place-items: center; border-bottom: 1px solid var(--line); font-size: 16px; }
.sftp-tools button { position: relative; min-height: 65px; display: flex; flex-direction: column; justify-content: center; align-items: center; gap: 4px; border: 0; color: #9eafb8; background: transparent; font-size: 12px; cursor: pointer; }
.sftp-tools button.active { color: #dce8ed; background: #17303d; }
.sftp-tools button.active::before { content: ""; position: absolute; left: 0; top: 0; bottom: 0; width: 2px; background: #55b8f8; }
.file-browser { min-width: 0; min-height: 0; display: flex; flex-direction: column; }''',
    '''.sftp-pane { position: relative; min-height: 0; display: grid; grid-template-columns: minmax(0, var(--sftp-tree-width, 224px)) 5px minmax(0, 1fr); background: var(--sftp); }
.sftp-tree-pane { min-width: 0; min-height: 0; display: flex; flex-direction: column; overflow: hidden; background: #0c202a; }
.sftp-tree-pane.collapsed { visibility: hidden; pointer-events: none; }
.sftp-tree-tabs { height: 39px; flex: none; display: flex; align-items: stretch; border-bottom: 1px solid var(--line); background: #102631; }
.sftp-tree-tabs > button:not(.sftp-tree-collapse) { min-width: 0; flex: 1; display: flex; align-items: center; justify-content: center; gap: 4px; padding: 0 5px; border: 0; color: #8ea2ad; background: transparent; cursor: pointer; font-size: 10px; }
.sftp-tree-tabs > button:not(.sftp-tree-collapse):hover { color: #d5e1e6; background: #17313d; }
.sftp-tree-tabs > button.active { color: #e1edf2; background: #183441; box-shadow: inset 0 -2px #58b7fb; }
.sftp-tree-collapse { width: 28px; flex: none; color: #8da1ab; border-left: 1px solid #2a414c; }
.sftp-tree-collapse:hover { color: #dce7ec; background: #1a3541; }
.sftp-tree-resizer { position: relative; z-index: 3; cursor: col-resize; background: #203641; }
.sftp-tree-resizer:hover, .sftp-tree-resizer:active { background: #4d91b8; }
.sftp-tree-expand { position: absolute; z-index: 6; left: 6px; top: 46px; width: 27px; height: 31px; color: #b9c8cf; background: #183441; border: 1px solid #385563; border-radius: 3px; }
.sftp-directory-tree { min-height: 0; flex: 1; display: flex; flex-direction: column; }
.sftp-directory-tree-header { height: 34px; flex: none; display: flex; align-items: center; padding: 0 7px 0 10px; color: #aebec5; border-bottom: 1px solid #263d48; font-size: 11px; }
.sftp-directory-tree-header strong { margin-right: auto; font-weight: 600; }
.sftp-directory-tree-header .icon-button { width: 26px; height: 27px; color: #8ea2ac; }
.sftp-directory-tree-header .icon-button:hover:not(:disabled) { color: #d9e6eb; background: #193641; }
.sftp-directory-tree-scroll { min-height: 0; flex: 1; padding: 4px 0 8px; overflow: auto; scrollbar-width: thin; scrollbar-color: #35505d transparent; }
.sftp-directory-tree-row { min-width: max-content; height: 28px; display: grid; grid-template-columns: 19px minmax(110px, 1fr) 20px; align-items: center; padding-left: calc(6px + (var(--tree-depth) * 16px)); color: #aebcc3; }
.sftp-directory-tree-row:hover { background: #132f3a; }
.sftp-directory-tree-row.selected { color: #edf5f8; background: #183a49; box-shadow: inset 2px 0 #58b7fb; }
.sftp-directory-tree-row.error { color: #e9a0a6; }
.sftp-directory-tree-toggle, .sftp-directory-tree-error { width: 19px; height: 25px; display: grid; place-items: center; padding: 0; border: 0; color: #8097a2; background: transparent; cursor: pointer; }
.sftp-directory-tree-toggle:disabled { opacity: .45; }
.sftp-directory-tree-toggle-spacer { width: 19px; }
.sftp-directory-tree-label { min-width: 0; height: 27px; display: flex; align-items: center; gap: 6px; padding: 0 5px 0 1px; border: 0; color: inherit; background: transparent; text-align: left; cursor: default; }
.sftp-directory-tree-label svg { flex: none; color: #ffc84f; fill: #ffc84f; }
.sftp-directory-tree-label span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.sftp-directory-tree-loading { color: #66b9e8; animation: sftp-tree-spin .8s linear infinite; }
.sftp-directory-tree-error { color: #ff8d95; }
@keyframes sftp-tree-spin { to { transform: rotate(360deg); } }
.sftp-directory-tree-empty { min-height: 0; flex: 1; display: grid; place-content: center; justify-items: center; gap: 7px; padding: 18px; color: #708690; text-align: center; }
.sftp-directory-tree-empty strong { color: #abb9c0; font-size: 12px; }
.sftp-directory-tree-empty span { font-size: 10px; line-height: 1.45; }
.sftp-tree-location-list { min-height: 0; flex: 1; overflow: auto; }
.sftp-tree-location-list > header { height: 34px; display: flex; align-items: center; gap: 8px; padding: 0 9px; color: #93a6af; border-bottom: 1px solid #263d48; font-size: 10px; }
.sftp-tree-location-list > header strong { margin-right: auto; }
.sftp-tree-location-list > header button { border: 0; color: #78bfe8; background: transparent; cursor: pointer; }
.sftp-tree-location-list > button { width: 100%; min-width: 0; height: 32px; display: grid; grid-template-columns: 20px minmax(0, 1fr) 20px; align-items: center; gap: 4px; padding: 0 7px; border: 0; border-bottom: 1px solid #203842; color: #aabac2; background: transparent; text-align: left; cursor: default; }
.sftp-tree-location-list > button:hover { background: #15313d; }
.sftp-tree-location-list > button span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.file-browser { min-width: 0; min-height: 0; display: flex; flex-direction: column; }''',
    "dual-pane tree styles",
)
style_path.write_text(style, encoding="utf-8")

# Documentation.
readme_path = Path("README.md")
readme = readme_path.read_text(encoding="utf-8")
readme = replace_once(
    readme,
    '''- 文件拖放仅在 SFTP 面板范围内生效，上传前展示服务器、目标路径、文件/目录数量、总大小和跳过项。

### 连接管理''',
    '''- 文件拖放仅在 SFTP 面板范围内生效，上传前展示服务器、目标路径、文件/目录数量、总大小和跳过项。
- 远程文件管理采用目录树与当前文件夹双栏布局；目录树按节点懒加载、按 SSH 会话隔离，并与右侧当前路径同步。

### 连接管理''',
    "README directory tree feature",
)
readme_path.write_text(readme, encoding="utf-8")

plan_path = Path("plan.md")
plan = plan_path.read_text(encoding="utf-8")
plan = replace_once(
    plan,
    '''状态：九个 SFTP 改造阶段已全部完成''',
    '''状态：原九阶段改造已完成；双栏文件管理 PR1 已实现，等待 CI、合并和本地验证''',
    "plan status",
)
plan = replace_once(
    plan,
    '''后续工作不再自动追加到本计划。发现本地验收缺陷时应新建小步修复 PR；新增远程编辑、预览、端口转发等能力时应先建立独立计划并重新确认安全边界。''',
    '''原九阶段计划到此结束。后续双栏文件管理按下述新顺序执行。''',
    "plan continuation marker",
)
plan += '''

## 17. SFTP 双栏文件管理改造

更新时间：2026-07-15  
当前状态：PR1 已实现，等待 CI、squash 合并和本地验证；PR2 尚未开始。

执行顺序：

1. PR1：远程目录树与双栏布局。
2. 本地验证目录树和多会话同步。
3. PR2：地址栏、工具栏和文件列表优化。
4. 完整 SFTP 实机验收。

### 17.1 PR1：远程目录树与双栏布局

分支：`feat/sftp-directory-tree`

实现范围：

- 左侧增加远程目录树，右侧始终显示当前打开目录的文件列表。
- 目录树按节点懒加载，不递归预读取整台服务器。
- Rust 新增只返回直接子目录的 `sftp_list_directories` 命令，并排除符号链接。
- 目录树状态、节点缓存、展开状态、选择路径和请求版本按 SSH 会话隔离。
- 右侧目录、地址栏和目录树选择双向同步；树同步失败不阻止右侧已成功打开的目录。
- 书签和历史移动到左侧导航区域，不删除现有能力。
- 左右面板增加可拖动分隔条，宽度保存到本地；目录树可折叠。
- 新增目录树状态、请求竞态、刷新复用、祖先展开和会话清理测试。

安全边界：

- 本 PR 只增加目录读取命令，不增加新的远程写入能力。
- 不跟随远程符号链接。
- 不修改传输队列、断点续传、目录替换、上传下载和删除语义。

本地验证门禁：

- `/`、`/root`、`/home`、`/var` 等路径的树与右侧列表同步。
- 两台服务器分别展开不同节点后快速切换，状态不得串线。
- 展开权限不足目录时只在对应节点显示错误，右侧当前目录保持可用。
- 右侧双击目录、地址栏跳转、前进后退后，树应展开并选中对应路径。
- 分隔条拖动、宽度持久化和目录树折叠恢复正常。
- 中文、空格、深层目录和高延迟服务器。

只有用户确认上述本地验证通过后，才开始 PR2 `feat/sftp-path-toolbar-polish`。

### 17.2 PR2：地址栏、工具栏和文件列表优化

状态：`等待 PR1 本地验证`

计划范围：

- 修正根节点重复分隔符，地址栏显示为 `/ > home > test` 的紧凑层级。
- 将导航与文件操作拆分分组，减少工具栏横向拥挤。
- 完善地址栏编辑态、错误保留和快捷键。
- 文件列表增加类型列，优化独立滚动和窄窗口布局。
- 完成后执行完整 SFTP 实机验收。
'''
plan_path.write_text(plan, encoding="utf-8")

handoff_path = Path("handoff.md")
handoff = handoff_path.read_text(encoding="utf-8")
handoff = replace_once(
    handoff,
    '''当前任务：九阶段 SFTP 改造计划已经完成，没有自动排定的下一 PR。''',
    '''当前任务：双栏文件管理 PR1“远程目录树与双栏布局”，分支 `feat/sftp-directory-tree`。实现完成并通过 CI、code review、squash 合并后，必须停在本地验证门禁；用户确认目录树与多会话同步无误后才开始 PR2。''',
    "handoff current task",
)
handoff += '''

## 9. 双栏文件管理后续顺序

1. PR1：远程目录树与双栏布局。
2. 用户本地验证目录树、双服务器隔离、权限错误和分隔条。
3. PR2：地址栏、工具栏和文件列表优化。
4. 用户完成完整 SFTP 实机验收。

PR1 关键文件：

- `src/components/sftp/SftpDirectoryTree.vue`
- `src/sftp/directory-tree-state.ts`
- `src/sftp/directory-tree-state.test.mjs`
- `src-tauri/src/sftp.rs`
- `src/App.vue`
- `src/styles.css`

PR1 不改变任何远程写入或传输协议行为。合并后不要自动开始 PR2，先等待用户给出本地验证结果。
'''
handoff_path.write_text(handoff, encoding="utf-8")
