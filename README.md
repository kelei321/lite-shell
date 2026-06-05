# LiteShell 轻壳

LiteShell 是一个轻量级 SSH / SFTP / 服务器管理客户端，目标是在保留 FinalShell 常用能力的同时，尽量降低内存占用和启动成本。

## 技术栈

- Tauri 2：跨平台桌面壳，使用系统 WebView，避免 Electron 内置 Chromium 带来的额外内存占用。
- Vue 3 + TypeScript：前端界面。
- xterm.js：终端渲染。
- Rust + ssh2：SSH / SFTP 后端能力。

## 第一阶段功能

- SSH 密码登录
- xterm.js 终端输入输出
- 终端窗口 resize
- 断开连接释放 SSH session
- SFTP 目录列表
- 简单快捷命令入口

## 开发命令

```bash
pnpm install
pnpm tauri dev
```

## 校验命令

```bash
pnpm build
cd src-tauri
cargo check
```

## 当前限制

- 第一版密码不落盘，每次连接时输入。
- 暂未支持密钥选择 UI，但 Rust 命令层已经预留 `privateKeyPath` / `passphrase` 参数。
- SFTP 当前只实现目录列表，上传、下载、删除、重命名后续补充。
