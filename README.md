# LiteShell

[![CI](https://github.com/kelei321/lite-shell/actions/workflows/ci.yml/badge.svg)](https://github.com/kelei321/lite-shell/actions/workflows/ci.yml)

LiteShell 是一款面向 Windows 桌面的轻量级 SSH 和 SFTP 客户端。项目使用 Vue 3、TypeScript、Tauri 2 和 Rust 构建，目标是在保持较低资源占用的同时，提供真实 SSH 终端、远程文件管理、服务器监控和连接配置管理能力。

项目目前处于持续开发阶段，核心功能已经可用，SFTP 后续可靠性和功能完善统一按照 [`plan.md`](./plan.md) 分阶段执行。

## 主要功能

### SSH

- 密码和私钥认证。
- PTY Shell、终端输入输出和窗口尺寸同步。
- 多会话标签管理。
- 首次连接主机密钥确认。
- 基于 `known_hosts` 的主机密钥校验和变更拦截。

### SFTP

- 浏览远程目录和文件属性。
- 文件上传、下载和多任务传输。
- 文件夹递归上传和下载。
- 新建目录、重命名、删除和递归删除。
- 覆盖、跳过和自动重命名冲突策略。
- 临时文件提交、失败重试和安全断点续传。
- Rust 后端持久传输队列，状态可跨应用重启恢复；默认并发 3，可设置 1～5。
- 目录上传和下载使用后端持久父批次，统一管理子任务、暂停/恢复、失败、提交、回滚和服务器重连。
- 暂停、继续、取消并保留断点、取消并删除断点、失败重试和清理已完成任务。
- 传输速度、进度、预计剩余时间、来源、目标、服务器和已续传字节展示。
- 路径书签、访问历史、搜索和排序。
- 每个 SSH 会话独立维护远程路径、目录列表、加载状态、错误、选择和前进后退历史。
- 迟到的目录请求会被 request version 守卫忽略，远程操作会校验选中项所属会话。
- 相同会话、方向和规范化目标路径的传输互斥，避免并发任务共享同一个 `.liteshell.part`。
- 文件传输会拒绝文件/目录类型不兼容的目标，并保护被目录占用的临时路径。
- 传输进入运行态后，成功、失败和取消统一产生唯一终态并清理资源。
- 断点续传使用稳定任务 ID、后端验证的 SSH 服务器身份和持久检查点，源身份不匹配时拒绝拼接。
- 递归上传下载由 Rust 安全扫描 manifest，默认跳过符号链接/junction，并限制深度、数量和累计大小。
- 文件冲突支持覆盖、跳过和重命名；目录冲突独立支持合并、跳过、重命名和 staging 式安全替换。
- 支持面包屑与可编辑路径、每会话独立导航、刷新保留有效选择、Ctrl/Shift 多选和批量删除。
- 支持文件右键菜单、隐藏文件开关和可配置的文件双击行为。
- 文件拖放仅在 SFTP 面板范围内生效，上传前展示服务器、目标路径、文件/目录数量、总大小和跳过项。
- 远程文件管理采用目录树与当前文件夹双栏布局；目录树按节点懒加载、按 SSH 会话隔离，并与右侧当前路径同步。
- 右侧列表、地址栏或历史导航进入父节点缓存中尚未出现的新目录时，目录树会补齐祖先关系并保持当前路径可见。
- 地址栏使用 `/ > home > ...` 紧凑层级，支持 `Ctrl+L` 或 `F6` 快速进入路径编辑；跳转失败时保留当前有效目录和错误提示。
- 导航、筛选设置和文件操作在工具栏中分组并支持窄窗口换行，减少横向滚动。
- 文件列表显示类型列，表头在独立滚动区域内保持可见，并针对窄窗口调整列宽。

### 连接管理

- 多级文件夹和收藏。
- 连接新增、编辑、复制、删除、批量移动和批量操作。
- LiteShell JSON 导入导出。
- OpenSSH config 和 FinalShell 配置导入。
- 密码和私钥口令保存到 Windows Credential Manager。

### 系统监控

- CPU 使用率和负载。
- 内存和交换分区。
- 网络收发速率。
- 磁盘使用情况。
- 系统运行时间和采样延迟。

系统监控只执行项目内固定的只读 Linux 命令，不允许前端动态拼接远程监控命令。

## 技术栈

- Vue 3
- TypeScript
- Vite 6
- xterm.js
- Tauri 2
- Rust
- `russh`
- `russh-sftp`
- Windows Credential Manager

## 项目结构

```text
.
├─ src/
│  ├─ App.vue                         主界面、会话、终端、SFTP 和系统监控
│  ├─ components/ConnectionManager.vue
│  ├─ services/ssh.ts                 前端 Tauri 命令和事件封装
│  ├─ sftp/transfer-queue.ts           后端队列快照、事件、等待和操作控制
│  ├─ sftp/transfer-queue-state.ts     队列事件与快照的单调合并逻辑
│  ├─ sftp/directory-batches.ts         目录批次快照、等待和父级操作控制
│  ├─ sftp/directory-batch-state.ts     批次事件与快照的单调合并逻辑
│  ├─ sftp/path-toolbar-shortcuts.ts   SFTP 路径编辑快捷键
│  ├─ main.ts
│  ├─ styles.css
│  └─ sftp-polish.css                 SFTP 地址栏、工具栏和文件列表覆盖样式
├─ src-tauri/
│  ├─ src/ssh.rs                      SSH 会话、认证和主机密钥校验
│  ├─ src/sftp.rs                     SFTP 浏览和安全文件传输核心
│  ├─ src/sftp_queue.rs               持久队列、调度、暂停、恢复和重试
│  ├─ src/sftp_batch.rs               持久目录批次、恢复、提交和回滚
│  ├─ src/atomic_file.rs              Windows 原子文件替换工具
│  ├─ src/monitor.rs                  Linux 系统监控
│  ├─ src/profiles.rs                 连接配置、凭据和导入导出
│  ├─ src/lib.rs                      Tauri 状态和命令注册
│  ├─ capabilities/default.json       Tauri 权限
│  └─ tauri.conf.json
├─ .github/workflows/ci.yml
├─ package.json
├─ plan.md                            SFTP 分阶段修改计划和进度
└─ handoff.md                         开发交接和当前上下文
```

## 开发环境

当前项目以 Windows 桌面环境为主要目标。

建议环境：

- Windows 10 或 Windows 11
- Node.js `24.16.0`
- npm `11` 或更高版本
- Rust stable
- WebView2 Runtime
- Visual Studio Build Tools，并安装 C++ 桌面开发组件

安装 Rust：

```powershell
winget install Rustlang.Rustup
rustup toolchain install stable --profile minimal --component rustfmt,clippy
rustup default stable
```

## 安装依赖

```powershell
npm ci
```

仓库提交了 `package-lock.json`，项目统一使用 npm，不要混用 Yarn 或 pnpm 更新锁文件。

## 启动开发环境

启动完整 Tauri 桌面应用：

```powershell
npm run desktop
```

只启动前端开发服务器：

```powershell
npm run dev
```

浏览器模式不能执行真实 SSH、SFTP、Windows 凭据和本地文件系统操作，只适合进行有限的前端布局检查。

## npm 命令

| 命令 | 作用 |
| --- | --- |
| `npm run dev` | 启动 Vite 开发服务器 |
| `npm run desktop` | 启动 Tauri 开发窗口 |
| `npm run build` | 构建前端生产资源 |
| `npm run desktop:build` | 执行 Tauri 桌面构建 |
| `npm run preview` | 预览前端生产构建 |
| `npm run typecheck` | 执行 Vue 和 TypeScript 类型检查 |
| `npm run format` | 格式化 Rust 代码 |
| `npm run format:check` | 检查 Rust 格式，不修改文件 |
| `npm run lint` | 使用 Clippy 阻断 correctness 和 suspicious 高风险问题 |
| `npm run test:frontend` | 使用 Node.js 原生测试运行器执行前端纯状态测试 |
| `npm run test:rust` | 运行 Rust 测试 |
| `npm run test` | 依次运行前端和 Rust 测试 |
| `npm run check` | 依次执行格式检查、Clippy 和类型检查 |
| `npm run validate` | 执行全部检查、测试和前端生产构建 |
| `npm run tauri -- <args>` | 透传参数到 Tauri CLI |

提交代码前至少执行：

```powershell
npm run validate
```

`npm run desktop:build` 会执行完整桌面构建，耗时和系统依赖要求高于常规校验，可在发布或安装包验证阶段单独运行。

## CI

GitHub Actions 在以下情况运行：

- 向 `main` 发起或更新 Pull Request。
- 向 `main` 推送提交。
- 手动触发工作流。

CI 使用 Windows runner，并执行：

1. `npm ci`
2. Rust `rustfmt` 检查
3. Rust Clippy correctness 和 suspicious 检查
4. Vue 和 TypeScript 类型检查
5. 前端纯状态测试
6. Rust 测试
7. Vite 前端生产构建

前端与 Rust 使用独立 job 并行运行，任一 job 失败不会跳过另一组校验。同一分支的新运行会取消旧运行，避免重复占用 CI 资源。

## SFTP 开发路线

SFTP 后续工作统一维护在 [`plan.md`](./plan.md)。执行顺序为：

1. 会话状态隔离（PR1 已完成）。
2. 相同目标路径传输互斥（PR2 已完成）。
3. 文件与目录冲突保护（PR3 已完成）。
4. 统一传输终态和清理（PR4 已完成）。
5. 安全断点续传和任务检查点（PR5 已完成）。
6. 递归传输和符号链接安全（PR6 已完成）。
7. 明确目录冲突语义（PR7 已完成）。
8. 后端统一传输队列、暂停和恢复（PR8 已完成）。
9. 导航与批量操作完善（PR9 已完成）。

双栏文件管理补充路线：PR1“远程目录树与双栏布局”和 PR2“地址栏、工具栏和文件列表优化”均已进入当前 `main` 基线。

一致性补强：当前独立 PR“持久化目录传输批次与目录替换事务”把目录级生命周期从前端迁移到 Rust 后端，并增加批量原子入队和重启恢复。

前五项属于数据安全和可靠性修复。在完成这些任务前，不优先扩展远程编辑、预览或更多批量写入能力。

每完成一个阶段，必须同步更新 `plan.md`、本 README 和 `handoff.md`。

## 数据与安全

### 连接配置

连接元数据保存到 Tauri 应用数据目录下的 `connections.json`。

配置包含连接名称、主机、端口、用户名、认证类型、私钥路径、文件夹和收藏状态，不应包含密码、私钥口令或私钥内容。

### 凭据

密码和私钥口令保存到 Windows Credential Manager。导出的 LiteShell JSON 不包含密码或私钥口令。

### 主机密钥

首次连接使用 TOFU 流程，用户确认后写入应用数据目录中的 `ssh/known_hosts`。已保存主机密钥发生变化时，连接会被阻止。

### 日志和提交

不得把以下内容写入源码、日志、终端输出、连接 JSON 或导出文件：

- SSH 密码。
- 私钥口令。
- 私钥正文。
- 真实生产环境凭据。

### 传输队列与目录批次

- `transfers/queue.json` 保存版本化文件任务队列；目录子任务通过可选 `batchId` 关联父批次。
- `transfers/batches.json` v1 保存目录批次、服务器身份、源/目标/实际写入目录、replacement、staging、backup、子任务计数和提交阶段。
- 队列、传输检查点和目录批次均先写临时文件，再使用 Windows `MoveFileExW` 原子替换；写盘失败不会提交对应内存快照。
- 恢复远程批次只按后端验证的稳定 `server_id` 匹配当前会话，不信任旧 `session_id`。
- 恢复或清理前会重新验证 replacement ID、目标与 staging/backup 的命名绑定、路径边界、方向和服务器身份。无法确定时进入 `rollback_required`，不会猜测性删除数据。

## 当前限制

- 主要支持 Windows 桌面环境。
- 后端队列会持久化 queued、paused 和 failed 任务；远程任务仍需重新连接相同后端验证服务器身份后才能继续。
- 暂停和“取消并保留”只有在磁盘上存在真实检查点时才标记为可恢复；源变化或检查点不一致仍会拒绝续传。
- 断点续传会校验后端 SSH 身份、源/目标路径、大小、纳秒级修改时间和首尾内容采样指纹；尚未执行全文件哈希。
- PR1～PR9 的 SFTP 可靠性与交互改造已完成；双栏文件管理 PR2 仍需在真实窗口中验证快捷键、工具栏换行、类型列和窄窗口滚动。
- 单次目录批次最多支持 5000 个文件；超过上限会在创建目标或 staging 前拒绝，不会部分入队。
- 目录替换复制阶段不会改动原目录；最终 rename 窗口退出后，持久批次会检查 target/staging/backup 的真实状态并继续提交、回滚或提示 `rollback_required`。
- 递归删除不是事务操作，中途失败时已经删除的文件无法自动恢复。
- 端口转发和终端编码切换尚未实现。
- 最新 SFTP 冲突策略、拖放、速度、ETA 和续传仍需要持续进行真实窗口和弱网场景验证。
- 当前 CI 构建前端资源，不生成和发布安装包。

### 目录批次本地验收

只在专用测试服务器目录执行以下写入验收：

1. 分别上传、下载含子目录的 merge、skip、rename、replace 批次。
2. 在 queued、running、paused 和 committing 状态退出应用，重启并重连同一服务器，确认父批次状态与文件任务一致。
3. replace 提交前后中断，确认原目录、staging 和 backup 不会被错误删除，必要时可点击“恢复原目录”。
4. 断开服务器后连接另一台服务器，确认不能接管原批次；重新连接相同已验证服务器后才能继续。
5. 验证 5000 文件边界、5001 文件提前拒绝、取消保留断点、取消删除断点、失败重试和回滚。
6. 验证 junction/symlink、权限不足、磁盘满、长路径和特殊字符。

## 开发约束

- 开始 SFTP 改动前必须阅读 `plan.md`，每个 PR 只完成一个计划任务。
- 保持应用轻量，不为连接管理器等功能新增独立 WebView。
- 不恢复 Mock SSH、Mock 终端、Mock 系统监控或演示会话。
- 不在普通 JSON 中保存密码或私钥口令。
- 服务器监控命令必须保持固定和只读。
- 修改应使用小范围 Pull Request，并在合并前通过 CI。
- 每个 SFTP PR 完成后同步更新 `plan.md`、`README.md` 和 `handoff.md`。

## 贡献流程

1. 阅读 `plan.md`、`README.md` 和 `handoff.md`。
2. 从最新 `main` 创建功能分支。
3. 只提交当前任务需要的文件。
4. 运行 `npm run validate`。
5. 比较 `main...branch`，确认没有无关改动。
6. 创建 Pull Request，并写明 Summary、Validation 和 Scope。
7. 完成 code review，等待 CI 通过后再合并。
8. 合并前确认三份项目文档已同步更新。
