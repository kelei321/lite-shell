# LiteShell 任务交接文档

更新时间：2026-07-16  
用户当前本地目录：`D:\Project\codex\lite-shell`  
仓库：`https://github.com/kelei321/lite-shell`

## 1. 文档使用规则

新会话或新任务开始前必须依次阅读：

1. `plan.md`：SFTP 后续开发的唯一执行计划和进度清单。
2. `README.md`：项目使用方式、能力、限制和开发约束。
3. `handoff.md`：当前代码状态、验证结果和最近上下文。

每完成一个 SFTP PR，必须在同一个 PR 中同步更新：

- `plan.md`
- `README.md`
- `handoff.md`

不得只修改代码而不更新项目状态文档。

## 2. 项目目标

LiteShell 是一款面向 Windows 桌面的轻量级 SSH 和 SFTP 客户端，界面参考 FinalShell 的高信息密度工作台，但不照搬其品牌和实现。

核心目标：

- 使用 Tauri 2 + Rust，保持较低内存占用。
- 提供真实 SSH 终端、SFTP、Linux 系统监控和连接管理。
- 使用主机密钥校验和 Windows Credential Manager 保护凭据。
- 不为连接管理器等功能新增独立 WebView。
- 默认深色、低干扰，终端保持最高视觉优先级。
- 密码和私钥口令不得写入普通 JSON、日志或导出文件。

技术栈：

- Vue 3
- TypeScript
- Vite 6
- xterm.js
- Tauri 2
- Rust stable
- `russh`
- `russh-sftp`
- Node.js 24.16.0
- npm

## 3. 当前阶段

项目已进入“核心功能可用，集中补强可靠性和交互”的阶段。

最新基础设施已经完成：

- 根目录已有完整 `README.md`。
- `package.json` 已补齐开发、校验、测试和构建命令。
- GitHub Actions 已拆分为 Frontend 和 Rust 两个 Windows job。
- CI 会运行前端类型检查、前端生产构建、Rust 格式检查、Rust 测试和 Clippy 高风险规则。

当前主线优先级已经切换到 SFTP 可靠性改造。

完整路线维护在 `plan.md`，共 9 个小步 PR：

1. SFTP 会话状态隔离。
2. 相同目标路径传输互斥。
3. 文件与目录冲突保护。
4. 统一传输终态和清理。
5. 安全断点续传和任务检查点。
6. 递归传输和符号链接安全。
7. 明确目录冲突语义。
8. 后端统一传输队列、暂停和恢复。
9. SFTP 导航与批量操作完善。

当前状态：PR1～PR9 和双栏文件管理改造已进入主线。当前分支 `fix/sftp-directory-batch-recovery` 正在完成独立 PR“持久化目录传输批次与目录替换事务”，尚未合并。

## 4. 已具备功能

### SSH

- Rust `russh` 真实连接后端。
- 密码和私钥认证。
- PTY Shell。
- 终端输入输出。
- 窗口尺寸同步。
- 主动断开。
- 主机密钥 TOFU 确认。
- `known_hosts` 持久化和主机密钥变更拦截。
- xterm.js 终端。
- 多会话标签。
- 最后一个会话关闭后返回快速连接页。
- 已移除 Mock SSH、Mock 输出和演示会话。

### SFTP

- 真实 SFTP 通道和目录读取。
- 文件、目录、符号链接、大小、修改时间和权限展示。
- 路径跳转、返回、前进、上级和刷新。
- 多文件上传。
- 单文件和批量下载。
- 本地目录递归上传。
- 远程目录递归下载。
- Rust 后端持久传输队列默认并发 3，可配置 1～5；排队任务不提前占用传输槽或目标锁。
- 新建目录。
- 重命名。
- 文件删除。
- 非空目录递归删除和二次确认。
- 递归删除保护根目录并检查服务器返回路径边界。
- `.liteshell.part` 临时文件。
- 覆盖时使用“备份原文件 -> 提交临时文件 -> 删除备份”的流程。
- 取消、失败重试和当前运行期断点续传。
- 速度、ETA、续传字节数和事件节流。
- 覆盖、跳过、自动重命名、取消冲突策略。
- 批量冲突支持“应用到全部”。
- Windows 文件和文件夹拖放上传。
- 当前目录搜索。
- 名称、大小和修改时间排序。
- 目录优先排序。
- 路径书签和访问历史。
- 较友好的中文错误提示。
- PR1：路径、列表、loading、错误、选择、历史、书签和最近路径按 SSH 会话隔离。
- PR1：目录请求使用 session/request version 守卫，关闭会话或新请求会使旧响应失效。
- PR1：远程下载、重命名、删除和上传目录目标使用条目或任务所属会话。
- PR2：上传和下载使用会话、方向、规范化目标路径组成的互斥键。
- PR2：RAII 目标锁保证成功、失败、取消和 panic 展开后自动释放。
- PR3：文件传输拒绝同名目录，目录传输拒绝同名文件，并保护 `.liteshell.part` 类型。
- PR4：运行态后的 shutdown、备份、提交、复制和取消路径统一发出终态并清理资源。
- PR5：稳定 taskId、后端验证的 SSH 身份、应用数据目录检查点和内容采样指纹，拒绝不安全续传。
- PR6：Rust 端受限递归 manifest、链接/junction 跳过、根边界、visited 集合、取消和扫描汇总；拖放扫描失败不会降级为文件上传。
- PR7：文件与目录冲突策略分离，目录支持合并/跳过/重命名/事务式替换，替换失败自动恢复原目录。
- PR8：后端是传输任务唯一事实来源，队列持久化 queued/paused/failed，支持暂停、恢复、取消保留/删除断点、失败重试和清理完成任务。
- PR8：任务事件与快照按单调版本合并，暂停和重启恢复以真实检查点为准；服务器重连后按稳定服务器身份自动唤醒任务。
- PR9：面包屑和可编辑路径、刷新保留有效选择、Ctrl/Shift 多选、批量删除、右键菜单、隐藏文件和文件双击设置。
- PR9：拖放命中限制在 SFTP 面板，上传预览绑定服务器和目标目录，并展示文件/目录数、总大小及跳过项。
- 双栏文件管理 PR1：左侧远程目录树按节点懒加载并按 SSH 会话隔离，右侧保持当前目录文件列表；地址栏、历史和右侧导航会同步选中树节点。
- 双栏文件管理 PR1 补强：父节点缓存已加载但缺少新目录时，选择当前路径会补齐父子关系，确保选中目录仍在左侧树中可见。
- 双栏文件管理 PR2：地址栏改为 `/ > home > ...` 紧凑层级，支持 `Ctrl+L`、`Command+L` 和 `F6` 进入路径编辑。
- 双栏文件管理 PR2：路径快捷键不会拦截终端或弹窗键盘行为，终端内 `Ctrl+L` 继续交给 SSH 会话。
- 双栏文件管理 PR2：工具栏支持分组和窄窗口换行；文件列表增加类型列、sticky 表头和独立滚动。

### 连接配置与凭据

- `connections.json` v2 存储结构。
- 多级 `ConnectionFolder` 和 `folderId`。
- v1 分组向 v2 文件夹自动迁移。
- 配置写入使用临时文件和 Windows 原子替换。
- 密码和私钥口令保存到 Windows Credential Manager。
- 导出不包含密码、私钥口令或私钥内容。

### 连接管理器

- 应用内大尺寸模态窗口，无新增 WebView。
- 全部连接、收藏和多级文件夹树。
- 文件夹新增、重命名、移动和删除。
- 循环父子关系拦截。
- 连接新增、编辑、复制、删除、收藏和移动。
- 搜索、表头排序、多选、拖放和右键菜单。
- 双击连接后关闭管理器并连接。
- 批量连接最多并发 3 个。
- LiteShell JSON 导入导出。
- OpenSSH config 导入。
- FinalShell 配置目录容错导入。
- 导入预览、重复连接跳过和失败警告。

### 系统监控

- 使用固定只读 Linux 命令采样。
- 不允许前端动态拼接监控命令。
- 解析 `/proc/stat`、`/proc/meminfo`、`/proc/net/dev`、`/proc/uptime` 和 `df`。
- CPU、内存、Swap、网络速率、磁盘、运行时间和采样延迟。

### 快速连接与 UI

- 无活动会话时显示快速连接表单。
- 支持主机、端口、用户名、密码、保存连接和保存密码。
- 展示已保存连接和连接管理器入口。
- 深色紧凑工作台布局。

## 5. SFTP 已确认风险

以下风险是 `plan.md` 的直接来源：

1. PR1 已处理 SFTP 多会话状态、迟到目录响应和选中项归属问题，仍需本地双会话实机确认。
2. PR2 已处理同一目标并发任务共享 `.liteshell.part` 的风险，仍需本地同目标并发实机确认。
3. PR3 已处理文件与目录类型冲突，仍需本地四类冲突实机确认。
4. PR4 已处理运行态后的终态和清理，仍需弱网、磁盘满和权限错误实机确认。
5. PR5 已实现稳定 taskId、后端会话身份、纳秒时间、内容采样指纹、持久检查点和重启识别；尚未执行全文件哈希。
6. PR6 已处理递归 symlink/junction、根边界、深度、数量、累计大小和取消；仍需 Windows 与真实服务器实机验证。
7. PR7 已拆分文件与目录冲突语义；仍需本地和远程 replace 实机验证。应用在最终目录切换窗口退出时，可能留下同级 staging/backup 残留，需要人工确认后清理。
8. PR8 已统一前后端传输事实来源；仍需本地验证暂停、重启、重连和多服务器队列行为。
9. PR9 已限制拖放命中范围并增加上传预览；不同 Windows DPI 和多显示器缩放仍需本地实机验证。

PR1～PR9 已完成核心数据安全、传输可靠性、递归边界、目录事务、后端队列和日常文件管理交互改造。

## 6. 当前任务与下一任务

当前任务：完成 `fix/sftp-directory-batch-recovery`，把目录上传/下载的父级生命周期、批量入队、目录替换 commit/rollback 和重启恢复迁移到 Rust 后端。当前尚未创建 PR，最终 PR 标题建议为 `fix: persist SFTP directory transfer batches`。

PR2 当前实现：

- 使用独立 `src/sftp/path-toolbar-shortcuts.ts` 管理 `Ctrl+L`、`Command+L` 和 `F6` 路径编辑快捷键。
- 快捷键作用域排除 `.terminal-host` 和 `.dialog-backdrop`，不抢占终端清屏或弹窗输入。
- 使用 `src/sftp-polish.css` 小范围覆盖现有 SFTP DOM，不重写 `App.vue`。
- 面包屑显示为 `/ > home > test`，路径编辑显示焦点状态和快捷键提示；跳转失败继续保留当前有效目录和错误信息。
- 工具栏允许分组换行，窄窗口不再依赖整条横向滚动。
- 文件列表增加类型列，使用独立滚动区、sticky 表头和响应式列宽。

自动化验证：

- 双栏文件管理 PR1 的 Windows GitHub Actions run `29402684994` 已通过前端纯状态测试、类型检查、生产构建、Rust 格式、测试和 Clippy。
- PR1 选中路径可见性补强的 GitHub Actions run `29419191950` 已通过 Frontend 与 Rust 全部检查。
- PR2 路径快捷键纯状态测试使用 Node.js 22.16.0 执行，3 项通过。
- PR #14 初始 GitHub Actions CI run `29467177443` 已通过：Vue/TypeScript 类型检查、Vite 生产构建、前端纯状态测试、Rust 格式、Rust 单元测试和 Clippy correctness/suspicious。
- `npm run validate` 未在当前环境作为单条命令执行；其组成检查已由上述 CI 覆盖。

尚需本地实机验证：

- `/`、`/root`、`/home`、`/var` 等路径的目录树与右侧列表同步。
- 双服务器分别展开不同节点后快速切换，路径、选择和展开状态不得串线。
- 地址栏根节点不得出现重复分隔符，深层路径显示为紧凑 `>` 层级。
- SFTP 文件区域外按 `Ctrl+L` 或 `F6` 后路径输入框获得焦点并全选。
- 终端内按 `Ctrl+L` 仍发送给 SSH 终端，不触发地址栏编辑；弹窗内快捷键同样不被接管。
- 路径跳转失败时当前有效目录不变，错误信息可见，再次编辑可正常恢复。
- 1366、1180、960 等宽度下工具栏换行、类型列、sticky 表头和横向/纵向滚动正常。
- 中文、空格、超长路径、大量文件列表和高延迟服务器。
- 不同 DPI/多显示器环境下拖放命中范围。
- 专用临时目录中的批量删除、目录递归删除和部分失败提示。
- 拖放上传预览的数量、大小、服务器和目标路径。
- PR1～PR8 在 `plan.md` 手工测试矩阵中列出的弱网、重启、链接和冲突场景。

PR2 当前状态为 `待验证`。最终文档 head 的 CI 和 code review 通过后可以合并；合并后进入完整 SFTP 实机验收，不继续追加新的 SFTP 功能。

## 7. 关键文件

### 项目文档和流程

- `plan.md`
  - SFTP 9 个 PR 和双栏文件管理 PR1/PR2 的范围、状态、测试和验收标准。
- `README.md`
  - 面向用户和贡献者的项目说明。
- `handoff.md`
  - 当前交接上下文和下一任务。
- `.github/workflows/ci.yml`
  - Frontend 与 Rust CI。
- `package.json`
  - npm scripts。
- `src/sftp/session-state.ts`
  - PR1 的会话状态、请求版本和选中项归属纯逻辑。
- `src/sftp/session-state.test.mjs`
  - Node.js 原生前端会话状态测试。
- `src/sftp/transfer-queue.ts`
  - PR8 后端队列快照、事件、等待器和用户操作控制。
- `src/sftp/transfer-queue-state.ts`
  - PR8 队列事件/快照的单调状态合并。
- `src/sftp/transfer-queue-state.test.mjs`
  - PR8 极快任务和快照竞态测试。
- `src/sftp/directory-batches.ts`
  - 目录批次快照、终态等待器和父批次统一操作。
- `src/sftp/directory-batch-state.ts`
  - 批次事件/快照单调合并与终态判定。
- `src/sftp/directory-batch-state.test.mjs`
  - 旧事件、多服务器隔离、重启状态和终态释放测试。
- `src/sftp/navigation-state.ts`
  - PR9 路径面包屑、选择协调、精确快照和拖放命中纯逻辑。
- `src/sftp/navigation-state.test.mjs`
  - PR9 导航、多选、刷新选择和拖放命中测试。
- `src/sftp/directory-tree-state.ts`
  - 双栏文件管理 PR1 的目录树节点缓存、请求版本、祖先同步、可见节点和会话清理逻辑。
- `src/sftp/directory-tree-state.test.mjs`
  - 目录树会话隔离、竞态、刷新、祖先展开和缓存过期路径可见性测试。
- `src/sftp/path-toolbar-shortcuts.ts`
  - 双栏文件管理 PR2 的路径编辑快捷键和终端/弹窗作用域保护。
- `src/sftp/path-toolbar-shortcuts.test.mjs`
  - PR2 快捷键识别、修饰键和作用域回归测试。

### 前端

- `src/App.vue`
  - 主界面、会话、终端、SFTP、系统监控、快速连接和当前传输 UI；PR2 不修改本文件业务逻辑。
- `src/styles.css`
  - 主界面、SFTP、拖放层、冲突对话框和传输状态基础样式。
- `src/sftp-polish.css`
  - PR2 地址栏、工具栏分组、类型列、滚动和窄窗口覆盖样式。
- `src/main.ts`
  - 加载 PR2 覆盖样式并注册路径快捷键。
- `src/components/ConnectionManager.vue`
  - 连接管理器。
- `src/services/ssh.ts`
  - Tauri 命令类型和前端接口。
- `vite.config.mjs`
  - 忽略 `src-tauri/target` 文件监听，避免 Windows EBUSY。

### Rust/Tauri

- `src-tauri/src/ssh.rs`
  - SSH 会话、认证、PTY、主机密钥和事件。
- `src-tauri/src/sftp.rs`
  - SFTP 浏览和安全文件传输核心、检查点及内部进度广播。
- `src-tauri/src/sftp_queue.rs`
  - PR8 持久队列、调度、暂停/恢复/取消/重试、重启迁移和状态测试。
- `src-tauri/src/sftp_batch.rs`
  - `batches.json` v1、目录批次状态机、两阶段创建/原子入队、恢复、提交、回滚和父级控制命令。
- `src-tauri/src/atomic_file.rs`
  - 队列、检查点和目录批次复用的 Windows `MoveFileExW` 原子替换。
- `src-tauri/src/sftp_recursive.rs`
  - PR6 本地/远程受限递归扫描、链接跳过、边界、限制、取消和汇总。
- `src-tauri/src/sftp_directory.rs`
  - PR7 文件/目录冲突分离和本地/远程事务式目录替换。
- `src-tauri/src/monitor.rs`
  - 固定只读系统监控命令和解析。
- `src-tauri/src/profiles.rs`
  - 连接存储、凭据、迁移、批量操作和导入导出。
- `src-tauri/src/lib.rs`
  - Tauri 状态和命令注册。
- `src-tauri/Cargo.toml`
  - Rust 依赖和 release 配置。
- `src-tauri/tauri.conf.json`
  - Tauri 窗口和开发命令。
- `src-tauri/capabilities/default.json`
  - Tauri 权限。

## 8. 开发和安全边界

- 默认使用中文回复。
- 修改前先说明本次 PR 的具体计划。
- 开始 SFTP 任务前必须阅读 `plan.md`。
- 每个 PR 只完成一个计划任务。
- 不使用破坏性 Git 命令，不覆盖无关修改。
- 不提交 `src-tauri/target`。
- 保持 Tauri 应用轻量，不新增大型状态库或独立 WebView，除非获得用户认可。
- 不恢复 Mock 终端、Mock 监控或演示会话。
- 密码、私钥口令和私钥内容不得写入 JSON、导出、日志或终端。
- 第三方导入不得读取、解密或迁移敏感凭据。
- 服务器监控命令必须固定且只读。
- 未经用户重新明确授权，不得在用户服务器执行上传、创建、重命名、删除、写文件或改权限。
- 真实 SFTP 写测试优先由用户在专用临时目录执行。

## 9. 开发环境

Windows 完整桌面开发需要：

- Node.js 24.16.0
- npm
- Rust stable MSVC 工具链
- `rustfmt`
- `clippy`
- Visual Studio Build Tools
- “使用 C++ 的桌面开发”工作负载
- MSVC v143 x64/x86
- Windows 10/11 SDK
- WebView2 Runtime

安装依赖：

```powershell
npm ci
```

启动桌面开发：

```powershell
npm run desktop
```

常规验证：

```powershell
npm run validate
```

完整安装包构建：

```powershell
npm run desktop:build
```

## 10. 已运行验证

基础设施 PR：

- PR #1：`docs: add README and complete CI validation`
- squash merge：`e2d0906229c08a5d95f01a1e329f7f11ccc66d62`

最近完成的 SFTP 可靠性阶段：

- PR1～PR9 已合并。
- 双栏文件管理 PR #12 已 squash 合并，合并提交为 `40502967e5874ed5e3b93e9c96862ab7c6ff3fbc`。
- PR #12 的 Windows GitHub Actions run `29402684994` 已通过 Frontend 与 Rust 全部检查。
- PR #13 的 Windows GitHub Actions run `29419191950` 已通过 Frontend 与 Rust 全部检查，合并提交为 `7490fc27979294d2f43c42950e834bc2cf249293`。
- PR #14 初始 GitHub Actions run `29467177443` 已通过 Frontend 与 Rust 全部检查。
- PR2 路径快捷键纯状态测试 3 项通过。

用户此前执行过真实 SSH 只读验证：

- `whoami`
- `pwd`
- `uname -a`
- `uptime`
- `df -h`
- `free -h`
- `ps`

尚未完成：

- `npm run build` 和完整 `npm run validate`：按项目规则等待用户明确允许。
- Git 范围检查、提交、推送、创建 PR 和 GitHub Actions。
- 真实 SFTP 目录批次写入、重启、重连、rename 中断和回滚验收。
- PR #14 文档更新后最终 head 的 GitHub Actions 校验。
- PR #14 code review 和 squash merge。
- PR2 地址栏、工具栏、类型列和窄窗口本地桌面验证。
- PR2 合并后的完整 SFTP 实机验收。
- Windows junction 循环实机测试。
- 真实服务器远程 symlink 越界与扫描取消测试。
- 最新 SFTP 写入、删除和弱网实机测试。
- 大文件、数千小文件、磁盘满、超长路径和特殊字符专项测试。
- 完整安装包验证。

## 11. PR 工作流

1. 从最新 `main` 创建计划指定分支。
2. 只修改当前任务所需文件。
3. 实现后运行相关测试和 `npm run validate`。
4. 同步更新 `plan.md`、`README.md` 和 `handoff.md`。
5. 比较 `main...branch`。
6. 创建 PR，描述包含 Summary、Validation、Scope。
7. code review 必须列出：
   - PR 状态。
   - head SHA。
   - changed files。
   - CI。
   - blockers。
   - non-blocking suggestions。
   - scope check。
   - 明确结论。
8. CI 通过且无阻塞问题后 squash merge。
9. 合并时使用 expected head SHA，并在合并后回查 merged、merge commit 和 merged_at。

## 12. 新会话启动提示词

```text
请继续开发 D:\Project\codex\lite-shell 项目。

开始前依次完整阅读：
1. plan.md
2. README.md
3. handoff.md

当前任务是完成 PR #14 `feat: polish SFTP path toolbar and file list`。分支为 feat/sftp-path-toolbar-polish。初始 CI run 29467177443 已通过；先核对文档更新后的最终 head、main...branch、changed files、CI 和 review threads。只有最终 CI 全部通过且 code review 结论为“可以合并”时，才使用 expected_head_sha squash merge。

PR2 只处理：
- 根节点重复分隔符和紧凑地址栏层级。
- 导航、筛选设置和文件操作工具栏分组及窄窗口换行。
- 地址栏编辑焦点、错误保留和 Ctrl+L/Command+L/F6 快捷键，并保留终端 Ctrl+L 行为。
- 文件列表类型列、sticky 表头、独立滚动和窄窗口布局。

要求：
- 默认中文回复。
- 不提前扩展传输队列、断点续传、冲突和远程写入语义。
- 不创建第二个功能分支。
- 完成 code review，等待最终 CI 通过后再合并。
- 合并必须使用 expected_head_sha，并回查 merged、merge_commit_sha 和 merged_at。
- 合并后进入完整 SFTP 实机验收，不继续新增 SFTP 功能。
- 未经明确授权，不得在用户服务器执行任何写操作。
```

## 13. 双栏文件管理后续顺序

1. PR1：远程目录树与双栏布局，已合并。
2. 用户已确认继续 PR2。
3. PR2：地址栏、工具栏和文件列表优化，当前待验证，PR #14 等待最终 CI、review 和合并。
4. PR2 合并后完成完整 SFTP 实机验收。

PR2 关键文件：

- `src/main.ts`
- `src/sftp-polish.css`
- `src/sftp/path-toolbar-shortcuts.ts`
- `src/sftp/path-toolbar-shortcuts.test.mjs`
- `README.md`
- `plan.md`
- `handoff.md`

双栏 PR2 不改变远程读写、传输队列、冲突或删除语义，当前 `main` 基线已包含该实现；本段以上旧 PR #14 信息仅保留为历史记录，当前任务以第 14 节目录批次 PR 为准。

## 14. 当前目录批次 PR 交接

分支：`fix/sftp-directory-batch-recovery`

新增命令：

- `sftp_batch_list`
- `sftp_batch_create`
- `sftp_batch_enqueue`
- `sftp_batch_pause`
- `sftp_batch_resume`
- `sftp_batch_retry`
- `sftp_batch_cancel`
- `sftp_batch_rollback`
- `sftp_batch_delete`
- `sftp_batch_wake`
- `sftp_queue_enqueue_batch`

核心数据：

- `SftpDirectoryBatch`：包含 batch/server/session、方向、源/目标/实际写入目录、冲突策略、replacement、staging/backup、子任务 ID 与计数、commit/rollback 标记、阶段、错误和时间戳。
- `TransferQueueTask.batch_id: Option<String>`：普通单文件为 `None`，目录子任务由后端写入父批次 ID。
- 存储位置：Tauri 应用数据目录 `transfers/batches.json`，版本 1。
- 单批上限：5000 文件；队列总上限保持 10000。

恢复规则：

- preparing 无子任务 → rollback_required；有已落盘子任务 → 通过 batch_id 重新关联。
- queued → 等待同一后端验证 `server_id` 的会话。
- running → 依据真实检查点转 paused 或 failed。
- committing → 检查 target/staging/backup 组合后幂等继续。
- 歧义状态、篡改路径或服务器身份不匹配 → rollback_required，不删除数据。

已执行：

- Node.js `24.16.0` 下 `npm ci` 通过。
- `npm run check` 通过（Rust 格式、Clippy correctness/suspicious、Vue 类型检查）。
- Rust 全量测试 53 项通过。
- `npm run test:frontend` 29 项通过。
- `npm run validate` 由用户在本地执行并报告通过（包含生产构建）。
- 未执行真实远程写入测试。

实机清单：

- upload/download × merge/skip/rename/replace。
- queued/running/paused/committing 退出与重启。
- target→backup、staging→target 阶段中断。
- 相同服务器重连与不同服务器拒绝接管。
- 5000/5001 文件门禁。
- 取消保留、取消删除、失败重试、恢复原目录。
- junction/symlink、权限不足、磁盘满、长路径、特殊字符。
