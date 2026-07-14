# LiteShell 任务交接文档

更新时间：2026-07-14  
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

当前状态：PR1～PR5 已合并；PR6 递归传输和符号链接安全已实现，CI 与 code review 已通过，等待合并。PR6 合并后进入 PR7。

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
- Rust `Semaphore` 全局限制最多 3 路传输。
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
7. 目录“覆盖”目前实际更接近合并，语义不准确。
8. 前端和后端各自维护部分传输队列状态，事实来源不统一。
9. 拖放监听整个 Tauri 窗口，不限于 SFTP 区域。

PR1～PR6 已完成核心数据安全、传输可靠性和递归边界改造；后续按计划处理目录冲突语义、统一队列和导航批量操作。

## 6. 当前任务与下一任务

当前任务：PR6 递归传输和符号链接安全，分支 `fix/sftp-recursive-transfer-safety`，状态为待验证，CI 与 code review 已通过。

PR6 已处理：

- 本地和远程递归目录扫描统一下沉到 Rust manifest 命令。
- 默认跳过本地符号链接、Windows junction/reparse point、远程符号链接和不支持条目。
- 使用 canonical path、visited 集合和根目录边界校验，防止循环和越界。
- 限制最大深度、文件数、目录数和累计大小。
- UI 显示扫描状态、汇总与跳过原因，并支持取消扫描。
- 取消标记在命令真正开始前也不会被清除，SFTP 通道打开失败仍会清理扫描状态。
- 拖放目录扫描失败不会降级成普通文件上传。

PR6 不处理：

- 文件与目录冲突策略的准确语义和安全目录替换。
- 后端统一传输队列、暂停与恢复。
- SFTP 导航、批量操作和拖放作用域完善。

PR6 合并后下一任务为 PR7：明确目录冲突语义，分支建议 `feat/sftp-directory-conflict-strategies`。

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
  - Node.js 原生前端状态测试。

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
  - SFTP 浏览、文件传输、冲突、续传、速度和 ETA。
- `src-tauri/src/sftp_recursive.rs`
  - PR6 本地/远程受限递归扫描、链接跳过、边界、限制、取消和汇总。
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

- PR1～PR5 已合并。
- PR #7（计划 PR5）squash merge：`5d2e9a197c44f841b95d3c4fb1d3d45649f4e184`。
- PR #8（计划 PR6）的实现已通过完整 Frontend 与 Rust CI；合并前仍需确认最新 head 的检查结果。

PR6 自动化测试覆盖：

- 递归深度、文件数量和累计大小限制。
- 远程根目录边界判断。
- 本地受限目录 manifest。
- 扫描开始前取消仍能生效。
- Unix 符号链接跳过。

用户此前执行过真实 SSH 只读验证：

- `whoami`
- `pwd`
- `uname -a`
- `uptime`
- `df -h`
- `free -h`
- `ps`

尚未完成：

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

当前优先任务是确认 PR #8（计划 PR6）已合并；若已合并，从最新 main 创建 `feat/sftp-directory-conflict-strategies`，开始 PR7：明确目录冲突语义。

PR7 只处理：
- 文件冲突继续使用覆盖、跳过、重命名。
- 目录冲突改为合并、跳过、重命名、替换。
- 文件和目录“应用于全部”策略分开。
- 目录替换必须二次确认并使用安全备份/提交流程。
- 服务器不支持安全 rename 时不得静默递归删除目标目录。

要求：
- 默认中文回复。
- 修改前先说明本次小步计划。
- 不提前实现 PR8 后端统一队列或 PR9 导航批量操作。
- 完成后运行测试和 `npm run validate`。
- 同步更新 plan.md、README.md、handoff.md。
- compare main...branch 后创建 PR，等待 code review 和 CI，通过后再合并。
- 未经明确授权，不得在用户服务器执行任何写操作。
```
