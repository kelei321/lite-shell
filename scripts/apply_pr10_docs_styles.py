from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, found {count}")
    return text.replace(old, new, 1)


# plan.md
path = Path("plan.md")
text = path.read_text(encoding="utf-8")
text = replace_once(text, "## 12. PR8：后端统一传输队列、暂停和恢复\n\n状态：`待开始`", "## 12. PR8：后端统一传输队列、暂停和恢复\n\n状态：`已完成`", "plan PR8 status")
text = replace_once(
    text,
    """### 完成记录

尚未开始。

---

## 13. PR9：SFTP 导航与批量操作完善""",
    """### 完成记录

实现内容：

- 新增 Rust 后端持久传输队列，任务保存到 Tauri 应用数据目录，后端统一创建、查询、调度和更新任务。
- 状态模型覆盖 `queued`、`running`、`pausing`、`paused`、`completed`、`failed` 和 `cancelled`。
- 默认并发为 3，可在 UI 中配置 1～5；排队任务在真正启动前不占用底层传输槽和目标写锁。
- 现有安全上传/下载核心、目标锁、稳定 taskId、服务器身份、检查点、临时文件提交和失败回滚全部继续复用，没有复制第二套传输实现。
- 前端删除 `transferTasks` Map 和独立检查点事实来源，改为消费后端队列快照与 `sftp-queue-task` 事件。
- 支持暂停、恢复、取消并保留断点、取消并删除断点、失败重试和清理已完成任务。
- 暂停、取消和重启恢复以磁盘上的真实检查点为准；没有检查点的运行任务不会被误报为可安全恢复。
- 应用重启会恢复 queued、paused 和 failed 任务；对应服务器重连后队列自动唤醒，同一服务器身份可跨 SSH 会话恢复。
- 旧版独立检查点会迁移为 paused 队列任务；上传/下载直传 Tauri 命令不再向前端暴露，递归扫描取消仍保留独立 operation 入口。
- 目录传输的文件子任务也进入后端队列；staging 目录必须等待全部子任务进入终态后才提交或回滚，目录子任务不提供单文件暂停。
- 队列事件和快照使用严格递增微秒版本合并，避免极快任务的 completed 状态被较旧 enqueue 响应或快照覆盖。
- 新增队列顺序、并发限制、服务器隔离、重启恢复、无检查点重启失败、暂停进度稳定、时间戳单调和前端快照竞态测试。

验证：GitHub Actions Windows run `29385233656` 已通过前端纯状态测试、Vue/TypeScript 类型检查、Vite 生产构建、Rust 格式、单元测试和 Clippy；未执行真实服务器写入测试。

本地待测：1～5 并发切换、运行中暂停/继续、取消保留/删除断点、失败重试、应用重启、服务器断开重连、多服务器隔离，以及目录 staging 子任务失败时的整体回滚。

下一步：PR9：SFTP 导航与批量操作完善。

---

## 13. PR9：SFTP 导航与批量操作完善""",
    "plan PR8 completion",
)
path.write_text(text, encoding="utf-8")

# README.md
path = Path("README.md")
text = path.read_text(encoding="utf-8")
text = replace_once(
    text,
    """- 临时文件提交、失败重试、取消和当前运行期断点续传。
- 传输速度、进度和预计剩余时间。""",
    """- 临时文件提交、失败重试和安全断点续传。
- Rust 后端持久传输队列，状态可跨应用重启恢复；默认并发 3，可设置 1～5。
- 暂停、继续、取消并保留断点、取消并删除断点、失败重试和清理已完成任务。
- 传输速度、进度、预计剩余时间、来源、目标、服务器和已续传字节展示。""",
    "README queue features",
)
text = replace_once(
    text,
    """│  ├─ services/ssh.ts                 前端 Tauri 命令和事件封装
│  ├─ main.ts""",
    """│  ├─ services/ssh.ts                 前端 Tauri 命令和事件封装
│  ├─ sftp/transfer-queue.ts           后端队列快照、事件、等待和操作控制
│  ├─ sftp/transfer-queue-state.ts     队列事件与快照的单调合并逻辑
│  ├─ main.ts""",
    "README frontend structure",
)
text = replace_once(
    text,
    """│  ├─ src/sftp.rs                     SFTP 和文件传输
│  ├─ src/monitor.rs""",
    """│  ├─ src/sftp.rs                     SFTP 浏览和安全文件传输核心
│  ├─ src/sftp_queue.rs               持久队列、调度、暂停、恢复和重试
│  ├─ src/monitor.rs""",
    "README Rust structure",
)
text = replace_once(
    text,
    """7. 明确目录冲突语义（PR7 已实现，等待 CI 和合并）。
8. 后端统一传输队列、暂停和恢复。
9. 导航与批量操作完善。""",
    """7. 明确目录冲突语义（PR7 已完成）。
8. 后端统一传输队列、暂停和恢复（PR8 已完成）。
9. 导航与批量操作完善（下一步）。""",
    "README roadmap",
)
text = replace_once(
    text,
    """- 应用重启后会识别未完成检查点，但仍需重新连接对应服务器后才能继续远程传输。
- 断点续传会校验后端 SSH 身份、源/目标路径、大小、纳秒级修改时间和首尾内容采样指纹；尚未执行全文件哈希。
- PR1～PR6 已完成；PR7 已实现目录冲突分离和 staging 式替换，CI 与 code review 已通过，等待合并。""",
    """- 后端队列会持久化 queued、paused 和 failed 任务；远程任务仍需重新连接相同后端验证服务器身份后才能继续。
- 暂停和“取消并保留”只有在磁盘上存在真实检查点时才标记为可恢复；源变化或检查点不一致仍会拒绝续传。
- 断点续传会校验后端 SSH 身份、源/目标路径、大小、纳秒级修改时间和首尾内容采样指纹；尚未执行全文件哈希。
- PR1～PR8 已完成可靠性改造，下一阶段为导航、批量操作和拖放作用域完善。""",
    "README limits",
)
path.write_text(text, encoding="utf-8")

# handoff.md
path = Path("handoff.md")
text = path.read_text(encoding="utf-8")
text = replace_once(text, "更新时间：2026-07-14", "更新时间：2026-07-15", "handoff date")
text = replace_once(
    text,
    "当前状态：PR1～PR6 已合并；PR7 目录冲突语义已实现，CI 与 code review 已通过，等待合并。PR7 合并后进入 PR8。",
    "当前状态：PR1～PR7 已合并；PR8 后端持久传输队列、暂停和恢复已实现，并通过 Windows 全量验证，等待最终标准 CI、code review 和合并。PR8 合并后进入 PR9。",
    "handoff current state",
)
text = replace_once(
    text,
    "- Rust `Semaphore` 全局限制最多 3 路传输。",
    "- Rust 后端持久传输队列默认并发 3，可配置 1～5；排队任务不提前占用传输槽或目标锁。",
    "handoff semaphore",
)
text = replace_once(
    text,
    """- PR7：文件与目录冲突策略分离，目录支持合并/跳过/重命名/事务式替换，替换失败自动恢复原目录。
""",
    """- PR7：文件与目录冲突策略分离，目录支持合并/跳过/重命名/事务式替换，替换失败自动恢复原目录。
- PR8：后端是传输任务唯一事实来源，队列持久化 queued/paused/failed，支持暂停、恢复、取消保留/删除断点、失败重试和清理完成任务。
- PR8：任务事件与快照按单调版本合并，暂停和重启恢复以真实检查点为准；服务器重连后按稳定服务器身份自动唤醒任务。
""",
    "handoff PR8 features",
)
text = replace_once(
    text,
    """8. 前端和后端各自维护部分传输队列状态，事实来源不统一。
9. 拖放监听整个 Tauri 窗口，不限于 SFTP 区域。

PR1～PR6 已完成核心数据安全、传输可靠性和递归边界改造；后续按计划处理目录冲突语义、统一队列和导航批量操作。""",
    """8. PR8 已统一前后端传输事实来源；仍需本地验证暂停、重启、重连和多服务器队列行为。
9. 拖放监听整个 Tauri 窗口，不限于 SFTP 区域，留到 PR9 处理。

PR1～PR8 已完成核心数据安全、传输可靠性、递归边界、目录事务和后端队列改造；后续按计划处理导航与批量操作。""",
    "handoff risks",
)
section_start = text.index("## 6. 当前任务与下一任务")
section_end = text.index("## 7. 关键文件", section_start)
section = """## 6. 当前任务与下一任务

当前任务：PR8 后端统一传输队列、暂停和恢复，分支 `feat/sftp-transfer-queue`，PR #10，代码和 Windows 全量验证已通过，等待最终标准 CI、code review 和 squash 合并。

PR8 已处理：

- Rust 后端持久化任务、并发设置、调度和完整状态机。
- 前端不再维护 `transferTasks` 或独立检查点列表作为事实来源。
- 默认并发 3，可配置 1～5；排队任务不提前占用传输槽和目标锁。
- 暂停、继续、取消并保留断点、取消并删除断点、失败重试和清理已完成任务。
- queued、paused 和 failed 任务跨应用重启恢复；服务器重连后按稳定服务器身份继续调度。
- 暂停/取消/重启状态以真实 checkpoint 文件为准，没有断点时不会显示为安全可恢复。
- 目录 staging 子任务也进入队列，并等待批次全部结束后提交或回滚。
- 极快任务的事件、enqueue 响应和列表快照使用单调时间版本合并，避免终态被旧状态覆盖。

PR8 不处理：

- 面包屑导航、可编辑路径体验和多选批量删除。
- SFTP 右键菜单和拖放作用域限制。
- 目录 replacement 事务本身的跨应用重启持久化；应用在最终切换窗口退出仍可能留下 staging/backup，需要人工确认。

PR8 合并后下一任务为 PR9：SFTP 导航与批量操作完善，分支建议 `feat/sftp-navigation-batch-operations`。

"""
text = text[:section_start] + section + text[section_end:]
text = replace_once(
    text,
    """- `src/sftp/session-state.test.mjs`
  - Node.js 原生前端状态测试。
""",
    """- `src/sftp/session-state.test.mjs`
  - Node.js 原生前端会话状态测试。
- `src/sftp/transfer-queue.ts`
  - PR8 后端队列快照、事件、等待器和用户操作控制。
- `src/sftp/transfer-queue-state.ts`
  - PR8 队列事件/快照的单调状态合并。
- `src/sftp/transfer-queue-state.test.mjs`
  - PR8 极快任务和快照竞态测试。
""",
    "handoff frontend key files",
)
text = replace_once(
    text,
    """- `src-tauri/src/sftp.rs`
  - SFTP 浏览、文件传输、冲突、续传、速度和 ETA。
- `src-tauri/src/sftp_recursive.rs`""",
    """- `src-tauri/src/sftp.rs`
  - SFTP 浏览和安全文件传输核心、检查点及内部进度广播。
- `src-tauri/src/sftp_queue.rs`
  - PR8 持久队列、调度、暂停/恢复/取消/重试、重启迁移和状态测试。
- `src-tauri/src/sftp_recursive.rs`""",
    "handoff Rust queue file",
)
path.write_text(text, encoding="utf-8")

# styles.css
path = Path("src/styles.css")
text = path.read_text(encoding="utf-8")
old = '''.transfer-queue { max-height: 126px; flex: none; overflow: auto; border-bottom: 1px solid #2c4450; }
.transfer-queue-heading { height: 27px; display: flex; align-items: center; justify-content: space-between; padding: 0 12px; color: #91a4ae; background: #102632; font-size: 11px; }
.transfer-queue-heading button { border: 0; color: #8ea1aa; background: transparent; cursor: pointer; font-size: 10px; }
.transfer-queue-heading button:hover { color: #d5e0e5; }
.upload-strip { min-height: 34px; display: grid; grid-template-columns: minmax(150px, auto) minmax(80px, 1fr) 160px 44px 38px; align-items: center; gap: 10px; padding: 0 12px; color: #aebcc3; background: #102834; border-bottom: 1px solid #2c4450; font-size: 11px; }
.upload-strip > span:first-child { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.upload-strip > span:first-child small { margin-left: 7px; color: #69bde9; }
.transfer-rate { color: #8097a2; text-align: right; }
.upload-strip > div { height: 4px; background: #263e49; }
.upload-strip i { display: block; height: 100%; background: #4caee9; }
.upload-strip strong { color: var(--green); font-weight: 500; }
.upload-strip.completed i { background: #66c97b; }
.upload-strip.failed i { background: #dc6871; }
.upload-strip.failed strong { color: #ff949c; }
.upload-strip.cancelled i { background: #7e8d95; }
.upload-strip.cancelled strong { color: #a5b1b7; }
.upload-strip > button { border: 0; color: #79bfe8; background: transparent; cursor: pointer; font-size: 10px; }
.checkpoint-queue { max-height: 178px; }
.checkpoint-queue .upload-strip { grid-template-columns: minmax(170px, auto) minmax(70px, 1fr) 132px 48px repeat(4, max-content); }
.checkpoint-queue .upload-strip > button:disabled { color: #5d7079; cursor: not-allowed; }
'''
new = '''.transfer-queue { max-height: 218px; flex: none; overflow: auto; border-bottom: 1px solid #2c4450; }
.transfer-queue-heading { position: sticky; z-index: 2; top: 0; height: 30px; display: flex; align-items: center; gap: 12px; padding: 0 12px; color: #91a4ae; background: #102632; font-size: 11px; }
.transfer-queue-heading > span { margin-right: auto; }
.transfer-queue-heading button { border: 0; color: #8ea1aa; background: transparent; cursor: pointer; font-size: 10px; }
.transfer-queue-heading button:hover { color: #d5e0e5; }
.transfer-concurrency { display: flex; align-items: center; gap: 5px; color: #80949e; }
.transfer-concurrency select { height: 22px; padding: 0 18px 0 6px; border: 1px solid #334d59; border-radius: 3px; color: #c2d0d6; background: #122c38; font-size: 10px; }
.upload-strip { min-width: 980px; min-height: 52px; display: grid; grid-template-columns: minmax(230px, 1.35fr) minmax(90px, .75fr) minmax(170px, .85fr) 60px; grid-auto-flow: column; grid-auto-columns: max-content; align-items: center; gap: 10px; padding: 5px 12px; color: #aebcc3; background: #102834; border-bottom: 1px solid #2c4450; font-size: 11px; }
.upload-strip > span:first-child { min-width: 0; display: grid; gap: 2px; overflow: hidden; }
.upload-strip > span:first-child > small { display: block; min-width: 0; margin: 0; overflow: hidden; color: #69bde9; text-overflow: ellipsis; white-space: nowrap; }
.upload-strip > span:first-child > small:first-of-type { color: #8199a5; }
.transfer-rate { min-width: 0; overflow: hidden; color: #8097a2; text-align: right; text-overflow: ellipsis; white-space: nowrap; }
.upload-strip > div { height: 4px; background: #263e49; }
.upload-strip i { display: block; height: 100%; background: #4caee9; }
.upload-strip strong { color: var(--green); font-weight: 500; white-space: nowrap; }
.upload-strip.queued strong { color: #89b6ce; }
.upload-strip.pausing strong, .upload-strip.paused strong { color: #f0bd68; }
.upload-strip.completed i { background: #66c97b; }
.upload-strip.failed i { background: #dc6871; }
.upload-strip.failed strong { color: #ff949c; }
.upload-strip.cancelled i { background: #7e8d95; }
.upload-strip.cancelled strong { color: #a5b1b7; }
.upload-strip > button { border: 0; color: #79bfe8; background: transparent; cursor: pointer; font-size: 10px; white-space: nowrap; }
.upload-strip > button:hover { color: #b8e2f8; }
.upload-strip > button.danger-button { color: #ff9ca4; }
'''
text = replace_once(text, old, new, "queue styles")
path.write_text(text, encoding="utf-8")
