# LiteShell

[![CI](https://github.com/kelei321/lite-shell/actions/workflows/ci.yml/badge.svg)](https://github.com/kelei321/lite-shell/actions/workflows/ci.yml)

LiteShell 是一款面向 Windows 桌面的轻量级 SSH 和 SFTP 客户端。项目使用 Vue 3、TypeScript、Tauri 2 和 Rust 构建，目标是在保持较低资源占用的同时，提供真实 SSH 终端、远程文件管理、服务器监控和连接配置管理能力。

项目目前处于持续开发阶段，核心功能已经可用，但仍需要继续补强传输可靠性、异常恢复和完整的实机验证。

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
└─ handoff.md
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
| `npm run test` | 运行 Rust 测试 |
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
5. Rust 测试
6. Vite 前端生产构建

前端与 Rust 使用独立 job 并行运行，任一 job 失败不会跳过另一组校验。同一分支的新运行会取消旧运行，避免重复占用 CI 资源。

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
- 传输任务和断点信息不会在应用重启后恢复。
- 当前断点续传依赖临时文件长度，尚未加入内容哈希校验。
- 递归删除不是事务操作，中途失败时已经删除的文件无法自动恢复。
- 端口转发和终端编码切换尚未实现。
- 最新 SFTP 冲突策略、拖放、速度、ETA 和续传仍需要持续进行真实窗口和弱网场景验证。
- 当前 CI 构建前端资源，不生成和发布安装包。

## 开发约束

- 保持应用轻量，不为连接管理器等功能新增独立 WebView。
- 不恢复 Mock SSH、Mock 终端、Mock 系统监控或演示会话。
- 不在普通 JSON 中保存密码或私钥口令。
- 服务器监控命令必须保持固定和只读。
- 修改应使用小范围 Pull Request，并在合并前通过 CI。

## 贡献流程

1. 从最新 `main` 创建功能分支。
2. 只提交当前任务需要的文件。
3. 运行 `npm run validate`。
4. 比较 `main...branch`，确认没有无关改动。
5. 创建 Pull Request，并写明 Summary、Validation 和 Scope。
6. 完成 code review，等待 CI 通过后再合并。
