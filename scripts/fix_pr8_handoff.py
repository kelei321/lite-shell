from pathlib import Path


path = Path("handoff.md")
text = path.read_text(encoding="utf-8")


def replace_once(old: str, new: str, label: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected one match, found {count}")
    text = text.replace(old, new, 1)


def replace_section(start: str, end: str, replacement: str, label: str) -> None:
    global text
    start_index = text.find(start)
    end_index = text.find(end, start_index + len(start))
    if start_index < 0 or end_index < 0:
        raise RuntimeError(f"{label}: section markers not found")
    text = text[:start_index] + replacement.rstrip() + "\n\n" + text[end_index:]


replace_once(
    "当前状态：PR1～PR5 已合并；PR6 递归传输和符号链接安全已实现，正在等待 CI、code review 和合并。PR6 合并后进入 PR7。",
    "当前状态：PR1～PR5 已合并；PR6 递归传输和符号链接安全已实现，CI 与 code review 已通过，等待合并。PR6 合并后进入 PR7。",
    "current status",
)

replace_once(
    "7. 目录“覆盖”目前实际更接近合并，语义不准确。\n9. 前端和后端各自维护部分传输队列状态，事实来源不统一。\n10. 拖放监听整个 Tauri 窗口，不限于 SFTP 区域。",
    "7. 目录“覆盖”目前实际更接近合并，语义不准确。\n8. 前端和后端各自维护部分传输队列状态，事实来源不统一。\n9. 拖放监听整个 Tauri 窗口，不限于 SFTP 区域。",
    "risk numbering",
)

replace_once(
    "PR1～PR5 完成前，不优先新增远程文件编辑、预览或更多批量写入能力。",
    "PR1～PR6 已完成核心数据安全、传输可靠性和递归边界改造；后续按计划处理目录冲突语义、统一队列和导航批量操作。",
    "completed reliability statement",
)

replace_section(
    "## 6. 当前任务与下一任务",
    "## 7. 关键文件",
    '''## 6. 当前任务与下一任务

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

PR6 合并后下一任务为 PR7：明确目录冲突语义，分支建议 `feat/sftp-directory-conflict-strategies`。''',
    "current task section",
)

replace_once(
    "- `src-tauri/src/sftp.rs`\n  - SFTP 浏览、传输、递归操作、冲突、续传、速度和 ETA。",
    "- `src-tauri/src/sftp.rs`\n  - SFTP 浏览、文件传输、冲突、续传、速度和 ETA。\n- `src-tauri/src/sftp_recursive.rs`\n  - PR6 本地/远程受限递归扫描、链接跳过、边界、限制、取消和汇总。",
    "recursive key file",
)

replace_section(
    "## 10. 已运行验证",
    "## 11. PR 工作流",
    '''## 10. 已运行验证

基础设施 PR：

- PR #1：`docs: add README and complete CI validation`
- squash merge：`e2d0906229c08a5d95f01a1e329f7f11ccc66d62`

最近完成的 SFTP 可靠性阶段：

- PR1～PR5 已合并。
- PR #7（计划 PR5）squash merge：`5d2e9a197c44f841b95d3c4fb1d3d45649f4e184`。
- PR #8（计划 PR6）最终 head：`bbcf5e3e1cd166601ead5cf1c1dd3ed8e1a4408e`。
- PR #8 CI run `29307771179` 已通过：
  - 前端状态测试、Vue/TypeScript 类型检查和 Vite 生产构建。
  - Rust 格式检查、Rust 单元测试和 Clippy correctness/suspicious。

PR6 新增测试覆盖：

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
- 完整安装包验证。''',
    "validation section",
)

replace_section(
    "## 12. 新会话启动提示词",
    "```text",
    '''## 12. 新会话启动提示词

```text
请继续开发 D:\\Project\\codex\\lite-shell 项目。

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
```''',
    "startup prompt",
)

path.write_text(text, encoding="utf-8", newline="\n")
Path("scripts/fix_pr8_handoff.py").unlink()
Path(".github/workflows/fix-pr8-handoff.yml").unlink()
