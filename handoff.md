# LiteShell 任务交接文档

更新时间：2026-07-15  
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

当前状态：PR1～PR9 已完成并进入主线，`plan.md` 中的九阶段 SFTP 改造计划已经结束。

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

当前任务：双栏文件管理 PR1 已合并，并已补强父节点缓存过期时的选中路径可见性；当前停在本地验证门禁，用户确认目录树和多会话同步无误后才开始 PR2。

PR9 已处理：

- 面包屑路径和独立编辑草稿。
- 每会话独立的路径、历史、隐藏文件状态、选择和范围选择锚点。
- 同目录刷新后保留仍有效选择。
- Ctrl/Command、Shift 和组合键多选。
- 批量下载、批量删除及文件/目录数量确认。
- 下载、重命名、删除、复制路径和刷新的右键菜单。
- 隐藏文件开关和文件双击行为设置。
- 只在 SFTP 区域接受拖放，并在上传前展示服务器、目标路径、文件数、目录数、总大小和跳过项。
- 状态栏中的项目数、已选数、目录文件总大小和运行传输数。

自动化验证：

- 双栏文件管理 PR1 的 Windows GitHub Actions run `29402684994` 已通过前端纯状态测试、类型检查、生产构建、Rust 格式、测试和 Clippy。
- 选中路径可见性补强使用 Node.js 22 运行目录树状态测试，7 项通过；完整类型检查、构建和 Rust 校验由当前 PR 的 CI 执行。

尚需本地实机验证：

- `/`、`/root`、`/home`、`/var` 等路径的目录树与右侧列表同步。
- 双服务器分别展开不同节点后快速切换，路径、选择和展开状态不得串线。
- 父节点已缓存后，通过地址栏或右侧列表进入新增目录，左树必须显示并选中当前目录。
- 权限不足节点只显示节点错误，右侧当前目录保持可用。
- 分隔条宽度持久化、目录树折叠恢复、中文、空格、深层目录和高延迟服务器。
- 不同 DPI/多显示器环境下拖放命中范围。
- 专用临时目录中的批量删除、目录递归删除和部分失败提示。
- 拖放上传预览的数量、大小、服务器和目标路径。
- PR1～PR8 在 `plan.md` 手工测试矩阵中列出的弱网、重启、链接和冲突场景。

在上述目录树与多会话验证通过前，不创建 PR2 `feat/sftp-path-toolbar-polish`。

## 7. 关键文件

### 项目文档和流程

- `plan.md`
  - SFTP 9 个 PR 的范围、状态、测试和验收标准。
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
- `src/sftp/navigation-state.ts`
  - PR9 路径面包屑、选择协调、精确快照和拖放命中纯逻辑。
- `src/sftp/navigation-state.test.mjs`
  - PR9 导航、多选、刷新选择和拖放命中测试。
- `src/sftp/directory-tree-state.ts`
  - 双栏文件管理 PR1 的目录树节点缓存、请求版本、祖先同步、可见节点和会话清理逻辑。
- `src/sftp/directory-tree-state.test.mjs`
  - 目录树会话隔离、竞态、刷新、祖先展开和缓存过期路径可见性测试。

### 前端

- `src/App.vue`
  - 主界面、会话、终端、SFTP、系统监控、快速连接和当前传输 UI。
- `src/styles.css`
  - 主界面、SFTP、拖放层、冲突对话框和传输状态样式。
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
- 目录树选中路径可见性补强的本地 Node.js 状态测试 7 项通过。

用户此前执行过真实 SSH 只读验证：

- `whoami`
- `pwd`
- `uname -a`
- `uptime`
- `df -h`
- `free -h`
- `ps`

尚未完成：

- 双栏目录树和多会话同步本地实机验收。
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

当前任务是完成双栏文件管理 PR1 的本地验证门禁。先确认目录树与右侧列表同步、多会话隔离、权限错误、分隔条和缓存过期路径可见性；只有用户确认通过后，才从最新 main 创建 `feat/sftp-path-toolbar-polish` 开始 PR2。

PR2 只处理：
- 根节点重复分隔符和紧凑地址栏层级。
- 导航与文件操作工具栏分组。
- 地址栏编辑态、错误保留和快捷键。
- 文件列表类型列、独立滚动和窄窗口布局。

要求：
- 默认中文回复。
- 修改前先说明本次小步计划。
- 不提前扩展传输队列、断点续传、冲突和远程写入语义。
- 完成后运行测试和 `npm run validate`。
- 同步更新 plan.md、README.md、handoff.md。
- compare main...branch 后创建 PR，等待 code review 和 CI，通过后再合并。
- 未经明确授权，不得在用户服务器执行任何写操作。
```

## 13. 双栏文件管理后续顺序

1. PR1：远程目录树与双栏布局。
2. 用户本地验证目录树、双服务器隔离、权限错误、缓存过期路径可见性和分隔条。
3. PR2：地址栏、工具栏和文件列表优化。
4. 用户完成完整 SFTP 实机验收。

PR1 关键文件：

- `src/components/sftp/SftpDirectoryTree.vue`
- `src/sftp/directory-tree-state.ts`
- `src/sftp/directory-tree-state.test.mjs`
- `src-tauri/src/sftp.rs`
- `src/App.vue`
- `src/styles.css`

PR1 不改变任何远程写入或传输协议行为。PR #12 已合并，Windows GitHub Actions run `29402684994` 已通过；选中路径可见性补强增加纯状态回归测试。当前必须停在本地验证门禁，用户确认通过后才开始 PR2。
