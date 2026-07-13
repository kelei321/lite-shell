<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { ask, open as openDialog, save } from "@tauri-apps/plugin-dialog";
import {
  IconChevronDown,
  IconChevronRight,
  IconCopy,
  IconDownload,
  IconFolder,
  IconFolderPlus,
  IconPlugConnected,
  IconSearch,
  IconServer,
  IconStar,
  IconStarFilled,
  IconTrash,
  IconUpload,
  IconX,
} from "@tabler/icons-vue";
import {
  applyConnectionsImport,
  batchProfiles,
  deleteFolder,
  describeCommandError,
  duplicateProfile,
  exportConnections,
  getConnectionManagerSnapshot,
  previewConnectionsImport,
  saveFolder,
  saveProfile,
  type ConnectionFolder,
  type ConnectionManagerSnapshot,
  type ConnectionProfile,
  type ImportPreview,
  type ImportSource,
  type ProfileAuthType,
} from "../services/ssh";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{
  close: [];
  changed: [snapshot: ConnectionManagerSnapshot];
  connect: [profiles: ConnectionProfile[]];
}>();

const snapshot = ref<ConnectionManagerSnapshot>({ version: 2, folders: [], profiles: [] });
const selectedFolder = ref("__all__");
const selectedIds = ref<string[]>([]);
const search = ref("");
const sortKey = ref<"name" | "host" | "username" | "updatedAt">("name");
const sortAscending = ref(true);
const expanded = ref(new Set<string>());
const busy = ref(false);
const error = ref("");
const importPreview = ref<ImportPreview | null>(null);
const draggedFolderId = ref("");
const draggedProfileIds = ref<string[]>([]);
const contextMenu = ref<{ x: number; y: number; type: "folder" | "profile"; id: string } | null>(null);

const form = ref({
  id: "",
  name: "",
  host: "",
  port: 22,
  username: "root",
  authType: "password" as ProfileAuthType,
  privateKeyPath: "",
  folderId: "folder-default",
  favorite: false,
  secret: "",
  rememberSecret: true,
  hasSecret: false,
});

type FlatFolder = ConnectionFolder & { depth: number; hasChildren: boolean };

const flatFolders = computed(() => {
  const result: FlatFolder[] = [];
  const append = (parentId: string | null, depth: number) => {
    const children = snapshot.value.folders
      .filter((folder) => folder.parentId === parentId)
      .sort((a, b) => a.sortOrder - b.sortOrder || a.name.localeCompare(b.name));
    for (const folder of children) {
      const hasChildren = snapshot.value.folders.some((child) => child.parentId === folder.id);
      result.push({ ...folder, depth, hasChildren });
      if (expanded.value.has(folder.id)) append(folder.id, depth + 1);
    }
  };
  append(null, 0);
  return result;
});

const visibleProfiles = computed(() => {
  const term = search.value.trim().toLowerCase();
  const values = snapshot.value.profiles.filter((profile) => {
    if (selectedFolder.value === "__favorites__" && !profile.favorite) return false;
    if (!selectedFolder.value.startsWith("__") && profile.folderId !== selectedFolder.value) return false;
    return !term || [profile.name, profile.host, profile.username].some((value) => value.toLowerCase().includes(term));
  });
  return [...values].sort((a, b) => {
    const left = String(a[sortKey.value]).toLowerCase();
    const right = String(b[sortKey.value]).toLowerCase();
    return left.localeCompare(right) * (sortAscending.value ? 1 : -1);
  });
});

const allVisibleSelected = computed(() => visibleProfiles.value.length > 0 && visibleProfiles.value.every((profile) => selectedIds.value.includes(profile.id)));
const selectedProfiles = computed(() => snapshot.value.profiles.filter((profile) => selectedIds.value.includes(profile.id)));

watch(() => props.open, (open) => {
  if (open) void reload();
});

onMounted(() => {
  if (props.open) void reload();
});

async function reload() {
  try {
    snapshot.value = await getConnectionManagerSnapshot();
    emit("changed", snapshot.value);
    for (const folder of snapshot.value.folders.filter((folder) => !folder.parentId)) expanded.value.add(folder.id);
  } catch (cause) {
    error.value = describeCommandError(cause);
  }
}

function toggleFolder(folder: ConnectionFolder) {
  const next = new Set(expanded.value);
  next.has(folder.id) ? next.delete(folder.id) : next.add(folder.id);
  expanded.value = next;
}

function countFolder(folderId: string) {
  return snapshot.value.profiles.filter((profile) => profile.folderId === folderId).length;
}

function selectProfile(profile: ConnectionProfile, checked = true) {
  selectedIds.value = checked
    ? [...new Set([...selectedIds.value, profile.id])]
    : selectedIds.value.filter((id) => id !== profile.id);
  editProfile(profile);
}

function toggleAllVisible() {
  if (allVisibleSelected.value) {
    const visible = new Set(visibleProfiles.value.map((profile) => profile.id));
    selectedIds.value = selectedIds.value.filter((id) => !visible.has(id));
  } else {
    selectedIds.value = [...new Set([...selectedIds.value, ...visibleProfiles.value.map((profile) => profile.id)])];
  }
}

function editProfile(profile?: ConnectionProfile) {
  form.value = profile ? {
    id: profile.id,
    name: profile.name,
    host: profile.host,
    port: profile.port,
    username: profile.username,
    authType: profile.authType,
    privateKeyPath: profile.privateKeyPath ?? "",
    folderId: profile.folderId,
    favorite: profile.favorite,
    secret: "",
    rememberSecret: profile.hasSecret,
    hasSecret: profile.hasSecret,
  } : {
    id: "",
    name: "",
    host: "",
    port: 22,
    username: "root",
    authType: "password",
    privateKeyPath: "",
    folderId: selectedFolder.value.startsWith("__") ? "folder-default" : selectedFolder.value,
    favorite: false,
    secret: "",
    rememberSecret: true,
    hasSecret: false,
  };
}

async function submitProfile() {
  busy.value = true;
  error.value = "";
  try {
    await saveProfile({
      id: form.value.id || undefined,
      name: form.value.name,
      host: form.value.host,
      port: Number(form.value.port),
      username: form.value.username,
      authType: form.value.authType,
      privateKeyPath: form.value.privateKeyPath || undefined,
      folderId: form.value.folderId,
      favorite: form.value.favorite,
      secret: form.value.secret || undefined,
      rememberSecret: form.value.rememberSecret,
    });
    form.value.secret = "";
    await reload();
  } catch (cause) {
    error.value = describeCommandError(cause);
  } finally {
    busy.value = false;
  }
}

async function createFolder(parentId?: string) {
  const name = window.prompt("文件夹名称");
  if (!name?.trim()) return;
  await saveFolder({ name: name.trim(), parentId });
  await reload();
}

async function renameFolder(folder: ConnectionFolder) {
  const name = window.prompt("新的文件夹名称", folder.name);
  if (!name?.trim() || name === folder.name) return;
  await saveFolder({ id: folder.id, name: name.trim(), parentId: folder.parentId ?? undefined });
  await reload();
}

async function removeFolder(folder: ConnectionFolder) {
  if (folder.id === "folder-default") return;
  const removeConnections = await ask("是否同时永久删除文件夹中的连接？选择“取消”可继续选择移动连接。", { title: "删除文件夹", kind: "warning", okLabel: "删除连接", cancelLabel: "不删除连接" });
  if (!removeConnections) {
    const move = await ask("将文件夹中的连接移动到默认分组并删除文件夹？", { title: "删除文件夹", kind: "warning", okLabel: "移动并删除", cancelLabel: "取消" });
    if (!move) return;
  }
  await deleteFolder(folder.id, removeConnections ? "delete_connections" : "move_to_default");
  selectedFolder.value = "__all__";
  await reload();
}

async function runBatch(action: Parameters<typeof batchProfiles>[1]) {
  if (!selectedIds.value.length) return;
  if (action.type === "delete") {
    const confirmed = await ask(`永久删除选中的 ${selectedIds.value.length} 个连接？`, { title: "批量删除", kind: "warning", okLabel: "删除", cancelLabel: "取消" });
    if (!confirmed) return;
  }
  snapshot.value = await batchProfiles(selectedIds.value, action);
  selectedIds.value = [];
  emit("changed", snapshot.value);
}

async function duplicateSelected() {
  if (selectedIds.value.length !== 1) return;
  const profile = await duplicateProfile(selectedIds.value[0]);
  await reload();
  selectedIds.value = [profile.id];
  editProfile(profile);
}

function startProfileDrag(profile: ConnectionProfile) {
  draggedProfileIds.value = selectedIds.value.includes(profile.id) ? selectedIds.value : [profile.id];
  draggedFolderId.value = "";
}

function startFolderDrag(folder: ConnectionFolder) {
  draggedFolderId.value = folder.id;
  draggedProfileIds.value = [];
}

async function dropOnFolder(folder: ConnectionFolder) {
  if (draggedProfileIds.value.length) {
    snapshot.value = await batchProfiles(draggedProfileIds.value, { type: "move", folderId: folder.id });
  } else if (draggedFolderId.value && draggedFolderId.value !== folder.id) {
    const source = snapshot.value.folders.find((item) => item.id === draggedFolderId.value);
    if (source) await saveFolder({ id: source.id, name: source.name, parentId: folder.id });
    await reload();
  }
  draggedProfileIds.value = [];
  draggedFolderId.value = "";
  emit("changed", snapshot.value);
}

async function chooseImport(source: ImportSource) {
  const path = source === "final_shell"
    ? await openDialog({ directory: true, multiple: false, title: "选择 FinalShell 配置目录" })
    : await openDialog({ directory: false, multiple: false, title: source === "open_ssh" ? "选择 OpenSSH config" : "选择 LiteShell 备份" });
  if (!path || Array.isArray(path)) return;
  importPreview.value = await previewConnectionsImport(source, path);
}

async function applyImport() {
  const preview = importPreview.value;
  if (!preview) return;
  const result = await applyConnectionsImport(preview.source, preview.path);
  importPreview.value = null;
  await reload();
  window.alert(`已导入 ${result.imported} 个连接，跳过重复 ${result.duplicates} 个，忽略 ${result.skipped} 项。`);
}

async function exportAll() {
  const path = await save({ title: "导出 LiteShell 连接", defaultPath: "liteshell-connections.json", filters: [{ name: "JSON", extensions: ["json"] }] });
  if (path) await exportConnections(path);
}

function changeSort(key: typeof sortKey.value) {
  if (sortKey.value === key) sortAscending.value = !sortAscending.value;
  else {
    sortKey.value = key;
    sortAscending.value = true;
  }
}

function openContextMenu(event: MouseEvent, type: "folder" | "profile", id: string) {
  contextMenu.value = { x: event.clientX, y: event.clientY, type, id };
  if (type === "profile" && !selectedIds.value.includes(id)) selectedIds.value = [id];
}

function contextFolder() {
  return snapshot.value.folders.find((folder) => folder.id === contextMenu.value?.id);
}

function contextProfile() {
  return snapshot.value.profiles.find((profile) => profile.id === contextMenu.value?.id);
}
</script>

<template>
  <div v-if="props.open" class="manager-backdrop">
    <section class="connection-manager" role="dialog" aria-modal="true" aria-label="连接管理器">
      <header class="manager-header"><div><IconServer :size="20" /><strong>连接管理器</strong></div><div class="manager-header-actions"><button @click="chooseImport('lite_shell')"><IconUpload :size="16" />导入 LiteShell</button><button @click="chooseImport('open_ssh')">导入 OpenSSH</button><button @click="chooseImport('final_shell')">导入 FinalShell</button><button @click="exportAll"><IconDownload :size="16" />导出</button><button class="icon-button" aria-label="关闭" @click="emit('close')"><IconX :size="20" /></button></div></header>
      <div class="manager-toolbar"><label><IconSearch :size="16" /><input v-model="search" placeholder="搜索名称、主机或用户" /></label><button @click="editProfile()">＋ 新建连接</button><button @click="createFolder(selectedFolder.startsWith('__') ? undefined : selectedFolder)"><IconFolderPlus :size="16" />新建文件夹</button><button :disabled="!selectedIds.length" @click="emit('connect', selectedProfiles)"><IconPlugConnected :size="16" />连接（{{ selectedIds.length }}）</button><button :disabled="selectedIds.length !== 1" @click="duplicateSelected"><IconCopy :size="16" />复制</button><button :disabled="!selectedIds.length" @click="runBatch({ type: 'favorite', favorite: true })"><IconStar :size="16" />收藏</button><button class="danger" :disabled="!selectedIds.length" @click="runBatch({ type: 'delete' })"><IconTrash :size="16" />删除</button></div>
      <div class="manager-body" @click="contextMenu = null">
        <aside class="folder-tree">
          <button :class="{ active: selectedFolder === '__all__' }" @click="selectedFolder = '__all__'"><IconServer :size="16" />全部连接 <small>{{ snapshot.profiles.length }}</small></button>
          <button :class="{ active: selectedFolder === '__favorites__' }" @click="selectedFolder = '__favorites__'"><IconStarFilled :size="16" />收藏 <small>{{ snapshot.profiles.filter(p => p.favorite).length }}</small></button>
          <div v-for="folder in flatFolders" :key="folder.id" class="folder-tree-row" :style="{ paddingLeft: `${8 + folder.depth * 18}px` }" draggable="true" @dragstart="startFolderDrag(folder)" @dragover.prevent @drop="dropOnFolder(folder)" @contextmenu.prevent="openContextMenu($event, 'folder', folder.id)">
            <button class="folder-toggle" @click="toggleFolder(folder)"><component :is="folder.hasChildren ? (expanded.has(folder.id) ? IconChevronDown : IconChevronRight) : IconFolder" :size="15" /></button>
            <button class="folder-name" :class="{ active: selectedFolder === folder.id }" @click="selectedFolder = folder.id"><span>{{ folder.name }}</span><small>{{ countFolder(folder.id) }}</small></button>
            <div class="folder-actions"><button @click="createFolder(folder.id)">＋</button><button v-if="folder.id !== 'folder-default'" @click="renameFolder(folder)">✎</button><button v-if="folder.id !== 'folder-default'" @click="removeFolder(folder)">×</button></div>
          </div>
        </aside>
        <main class="manager-list">
          <div class="manager-table manager-table-head"><span><input type="checkbox" :checked="allVisibleSelected" @change="toggleAllVisible" /></span><button @click="changeSort('name')">名称</button><button @click="changeSort('host')">主机</button><button @click="changeSort('username')">用户</button><span>端口</span><span>认证</span><button @click="changeSort('updatedAt')">更新时间</button></div>
          <div class="manager-list-scroll">
            <div v-for="profile in visibleProfiles" :key="profile.id" class="manager-table manager-table-row" :class="{ selected: selectedIds.includes(profile.id) }" draggable="true" @dragstart="startProfileDrag(profile)" @dblclick="emit('connect', [profile])" @contextmenu.prevent="openContextMenu($event, 'profile', profile.id)"><span><input type="checkbox" :checked="selectedIds.includes(profile.id)" @change="selectProfile(profile, ($event.target as HTMLInputElement).checked)" /></span><button @click="selectProfile(profile)"><component :is="profile.favorite ? IconStarFilled : IconServer" :size="15" /><strong>{{ profile.name }}</strong></button><span>{{ profile.host }}</span><span>{{ profile.username }}</span><span>{{ profile.port }}</span><span>{{ profile.authType === 'password' ? '密码' : '私钥' }}</span><span>{{ new Date(profile.updatedAt * 1000).toLocaleDateString() }}</span></div>
            <div v-if="!visibleProfiles.length" class="manager-empty">没有匹配的连接</div>
          </div>
        </main>
        <aside class="manager-detail">
          <h3>{{ form.id ? '编辑连接' : '新建连接' }}</h3>
          <label><span>名称</span><input v-model="form.name" /></label>
          <label><span>主机</span><input v-model="form.host" /></label>
          <div class="detail-pair"><label><span>端口</span><input v-model.number="form.port" type="number" min="1" max="65535" /></label><label><span>用户名</span><input v-model="form.username" /></label></div>
          <label><span>文件夹</span><select v-model="form.folderId"><option v-for="folder in flatFolders" :key="folder.id" :value="folder.id">{{ '　'.repeat(folder.depth) }}{{ folder.name }}</option></select></label>
          <fieldset><legend>认证</legend><label><input v-model="form.authType" type="radio" value="password" />密码</label><label><input v-model="form.authType" type="radio" value="private_key" />私钥</label></fieldset>
          <label v-if="form.authType === 'private_key'"><span>私钥路径</span><input v-model="form.privateKeyPath" /></label>
          <label><span>{{ form.authType === 'password' ? '密码' : '私钥口令' }}</span><input v-model="form.secret" type="password" :placeholder="form.hasSecret ? '已安全保存，留空保持不变' : ''" /></label>
          <label class="detail-check"><input v-model="form.rememberSecret" type="checkbox" />保存到 Windows 凭据管理器</label>
          <label class="detail-check"><input v-model="form.favorite" type="checkbox" />收藏连接</label>
          <p v-if="error" class="manager-error">{{ error }}</p>
          <button class="detail-save" :disabled="busy" @click="submitProfile">{{ busy ? '保存中…' : '保存连接' }}</button>
        </aside>
      </div>
      <div v-if="contextMenu" class="manager-context-menu" :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }" @click.stop>
        <template v-if="contextMenu.type === 'profile'">
          <button @click="contextProfile() && emit('connect', [contextProfile()!]); contextMenu = null">连接</button>
          <button @click="duplicateSelected(); contextMenu = null">复制连接</button>
          <button @click="runBatch({ type: 'favorite', favorite: true }); contextMenu = null">收藏</button>
          <button class="danger" @click="runBatch({ type: 'delete' }); contextMenu = null">删除</button>
        </template>
        <template v-else-if="contextFolder()">
          <button @click="createFolder(contextFolder()!.id); contextMenu = null">新建子文件夹</button>
          <button v-if="contextFolder()!.id !== 'folder-default'" @click="renameFolder(contextFolder()!); contextMenu = null">重命名</button>
          <button v-if="contextFolder()!.id !== 'folder-default'" class="danger" @click="removeFolder(contextFolder()!); contextMenu = null">删除文件夹</button>
        </template>
      </div>
      <footer class="manager-footer"><span>{{ snapshot.folders.length }} 个文件夹，{{ snapshot.profiles.length }} 个连接</span><span>双击连接可立即打开会话</span></footer>
    </section>
    <div v-if="importPreview" class="import-preview"><section><h3>导入预览</h3><dl><div><dt>新增连接</dt><dd>{{ importPreview.newCount }}</dd></div><div><dt>重复跳过</dt><dd>{{ importPreview.duplicateCount }}</dd></div><div><dt>无法识别</dt><dd>{{ importPreview.skippedCount }}</dd></div></dl><p v-if="importPreview.warnings.length">{{ importPreview.warnings.slice(0, 5).join('\n') }}</p><footer><button @click="importPreview = null">取消</button><button class="primary" @click="applyImport">确认导入</button></footer></section></div>
  </div>
</template>

<style scoped>
.manager-backdrop{position:fixed;z-index:80;inset:0;display:grid;place-items:center;padding:24px;background:rgba(2,8,12,.78)}
.connection-manager{width:min(1320px,96vw);height:min(850px,94vh);display:grid;grid-template-rows:50px 45px minmax(0,1fr) 32px;color:#dce5ea;background:#0d202a;border:1px solid #3a5360;border-radius:6px;box-shadow:0 22px 70px rgba(0,0,0,.55);overflow:hidden}
.manager-header,.manager-toolbar,.manager-footer{display:flex;align-items:center;border-bottom:1px solid #2b424d}.manager-header{padding:0 14px 0 18px;background:#142c38}.manager-header>div{display:flex;align-items:center;gap:9px}.manager-header-actions{margin-left:auto}.manager-header button,.manager-toolbar button{height:30px;display:flex;align-items:center;gap:5px;padding:0 9px;border:1px solid #334b57;border-radius:3px;color:#c3d0d6;background:#18323f;cursor:pointer}.manager-header .icon-button{width:32px;padding:0;border:0;background:transparent}.manager-toolbar{gap:7px;padding:0 10px;background:#102630}.manager-toolbar label{width:280px;height:31px;display:flex;align-items:center;gap:7px;padding:0 9px;border:1px solid #334b57;background:#0b1d26}.manager-toolbar input{min-width:0;flex:1;border:0;outline:0;color:#dce5ea;background:transparent}.manager-toolbar .danger{margin-left:auto;color:#ffabb1}.manager-toolbar button:disabled{opacity:.4;cursor:default}
.manager-body{min-height:0;display:grid;grid-template-columns:235px minmax(440px,1fr) 310px}.folder-tree{min-height:0;padding:9px 6px;overflow:auto;background:#102733;border-right:1px solid #2b424d}.folder-tree>button{width:100%;height:34px;display:flex;align-items:center;gap:8px;padding:0 9px;border:0;color:#aebcc3;background:transparent;text-align:left;cursor:pointer}.folder-tree>button small,.folder-name small{margin-left:auto;color:#718690}.folder-tree button.active{color:#e8f0f3;background:#1b3a49}.folder-tree-row{height:33px;display:flex;align-items:center}.folder-toggle{width:23px;border:0;color:#80939d;background:transparent;cursor:pointer}.folder-name{min-width:0;height:30px;flex:1;display:flex;align-items:center;padding:0 5px;border:0;color:#b8c5cb;background:transparent;cursor:pointer}.folder-name span{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.folder-actions{display:none}.folder-tree-row:hover .folder-actions{display:flex}.folder-actions button{width:20px;border:0;color:#8498a2;background:transparent;cursor:pointer}
.manager-list{min-width:0;min-height:0;display:flex;flex-direction:column;background:#0b1d26;border-right:1px solid #2b424d}.manager-table{min-width:720px;display:grid;grid-template-columns:34px minmax(150px,1.25fr) minmax(130px,1fr) minmax(90px,.7fr) 60px 70px 90px;align-items:center}.manager-table-head{height:35px;color:#91a4ae;background:#122a35;border-bottom:1px solid #2a414c;font-size:11px}.manager-table-head button{height:100%;border:0;color:inherit;background:transparent;text-align:left;cursor:pointer}.manager-list-scroll{min-height:0;flex:1;overflow:auto}.manager-table-row{height:39px;color:#b8c5cb;border-bottom:1px solid #203640;font-size:11px}.manager-table-row.selected{background:#173746;box-shadow:inset 2px 0 #58b7fb}.manager-table-row>span,.manager-table-row>button{min-width:0;padding:0 8px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.manager-table-row>button{height:100%;display:flex;align-items:center;gap:7px;border:0;color:inherit;background:transparent;text-align:left;cursor:pointer}.manager-table-row>button svg{color:#70bff0}.manager-empty{padding:40px;color:#718690;text-align:center}
.manager-detail{min-height:0;padding:15px 16px;overflow:auto;background:#112732}.manager-detail h3{margin:0 0 14px;font-size:15px}.manager-detail>label,.detail-pair label{display:grid;gap:5px;margin-bottom:11px;color:#9fb0b8;font-size:11px}.manager-detail input:not([type=checkbox]):not([type=radio]),.manager-detail select{width:100%;height:33px;padding:0 8px;border:1px solid #38505c;border-radius:3px;outline:0;color:#dce5ea;background:#0a1c25}.detail-pair{display:grid;grid-template-columns:90px 1fr;gap:9px}.manager-detail fieldset{display:flex;gap:22px;margin:0 0 11px;padding:8px 10px;border:1px solid #344b57}.manager-detail fieldset label,.detail-check{display:flex!important;align-items:center;gap:7px}.manager-detail fieldset legend{color:#9fb0b8;font-size:11px}.detail-save{width:100%;height:35px;margin-top:8px;border:1px solid #359bd8;border-radius:3px;color:white;background:#258ac6;cursor:pointer}.manager-error{color:#ff9da5;font-size:11px}.manager-footer{justify-content:space-between;padding:0 12px;color:#7f939e;background:#0d2029;border-top:1px solid #2b424d;border-bottom:0;font-size:10px}
.import-preview{position:absolute;z-index:2;inset:0;display:grid;place-items:center;background:rgba(0,0,0,.55)}.import-preview section{width:420px;padding:18px;color:#dce5ea;background:#142c38;border:1px solid #405965;border-radius:5px}.import-preview h3{margin:0 0 14px}.import-preview dl div{display:flex;justify-content:space-between;padding:7px 0;border-bottom:1px solid #2d4651}.import-preview dd{margin:0;color:#78dc53}.import-preview p{white-space:pre-line;color:#ffb0b5;font-size:11px}.import-preview footer{display:flex;justify-content:flex-end;gap:8px;margin-top:16px}.import-preview button{height:33px;padding:0 14px;border:1px solid #3a515d;color:#c5d0d6;background:#1a303b}.import-preview .primary{color:white;background:#258ac6}
.manager-context-menu{position:fixed;z-index:100;min-width:145px;display:grid;padding:5px;background:#17313e;border:1px solid #405965;border-radius:4px;box-shadow:0 10px 28px rgba(0,0,0,.45)}.manager-context-menu button{height:30px;padding:0 10px;border:0;color:#c6d2d8;background:transparent;text-align:left;cursor:pointer}.manager-context-menu button:hover{background:#244451}.manager-context-menu button.danger{color:#ff9da5}
</style>
