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
- 临时文件提交、失败重试、取消和当前运行期断点续传。
- 传输速度、进度和预计剩余时间。
- 路径书签、访问历史、搜索和排序。
- 每个 SSH 会话独立维护远程路径、目录列表、加载状态、错误、选择和前进后退历史。
- 迟到的目录请求会被 request version 守卫忽略，远程操作会校验选中项所属会话。
- 相同会话、方向和规范化目标路径的传输互斥，避免并发任务共享同一个 `.liteshell.part`。
- 文件传输会拒绝文件/目录类型不兼容的目标，并保护被目录占用的临时路径。
- 传输进入运行态后，成功、失败和取消统一产生唯一终态并清理资源。
- 断点续传使用稳定任务 ID、后端验证的 SSH 服务器身份和持久检查点，源身份不匹配时拒绝拼接。
- 递归上传下载由 Rust 安全扫描 manifest，默认跳过符号链接/junction，并限制深度、数量和累计大小。
- 文件冲突支持覆盖、跳过和重命名；目录冲突独立支持合并、跳过、重命名和 staging 式安全替换。

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
│  ├─ main.ts
│  └─ styles.css
├─ src-tauri/
│  ├─ src/ssh.rs                      SSH 会话、认证和主机密钥校验
│  ├─ src/sftp.rs                     SFTP 和文件传输
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
7. 明确目录冲突语义（PR7 已实现，等待 CI 和合并）。
8. 后端统一传输队列、暂停和恢复。
9. 导航与批量操作完善。

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

## 当前限制

- 主要支持 Windows 桌面环境。
- 应用重启后会识别未完成检查点，但仍需重新连接对应服务器后才能继续远程传输。
- 断点续传会校验后端 SSH 身份、源/目标路径、大小、纳秒级修改时间和首尾内容采样指纹；尚未执行全文件哈希。
- PR1～PR6 已完成；PR7 已实现目录冲突分离和 staging 式替换，正在等待 CI、code review 和合并。
- 目录替换复制阶段不会改动原目录；若应用恰好在最终 rename 切换窗口退出，可能留下同级 staging/backup 残留，需要人工确认后清理。
- 递归删除不是事务操作，中途失败时已经删除的文件无法自动恢复。
- 端口转发和终端编码切换尚未实现。
- 最新 SFTP 冲突策略、拖放、速度、ETA 和续传仍需要持续进行真实窗口和弱网场景验证。
- 当前 CI 构建前端资源，不生成和发布安装包。

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
