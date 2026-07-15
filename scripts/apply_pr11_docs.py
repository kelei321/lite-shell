from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, found {count}")
    return text.replace(old, new, 1)


plan_path = Path("plan.md")
plan = plan_path.read_text(encoding="utf-8")
plan = replace_once(
    plan,
    "更新时间：2026-07-14  \n状态：实施进行中，PR1～PR6 已完成，PR7 已通过验证并等待合并",
    "更新时间：2026-07-15  \n状态：九个 SFTP 改造阶段已全部完成",
    "plan header",
)
for heading in [
    "## 9. PR5：安全断点续传和任务检查点",
    "## 10. PR6：递归传输和符号链接安全",
    "## 11. PR7：明确目录冲突语义",
]:
    marker = f"{heading}\n\n状态：`待验证`"
    plan = replace_once(plan, marker, f"{heading}\n\n状态：`已完成`", f"{heading} status")
plan = replace_once(
    plan,
    "## 13. PR9：SFTP 导航与批量操作完善\n\n状态：`待开始`",
    "## 13. PR9：SFTP 导航与批量操作完善\n\n状态：`已完成`",
    "PR9 status",
)
plan = replace_once(
    plan,
    "### 完成记录\n\n尚未开始。\n\n---\n\n## 14. 自动化测试计划",
    """### 完成记录

合并 PR：#11  
完成日期：2026-07-15

实现内容：

- 新增远程路径面包屑和独立路径编辑草稿，路径跳转失败不会覆盖当前有效目录。
- 刷新同一目录后保留仍存在的选择，并使用最新目录条目对象替换旧引用。
- 支持 Ctrl/Command 切换选择、Shift 连续选择和 Ctrl/Command+Shift 追加范围，选择锚点按 SSH 会话隔离。
- 批量删除在确认框中显示文件/链接与目录数量，执行前要求会话和完整选择快照保持一致，并汇总部分失败。
- 增加右键菜单的下载、重命名、删除、复制路径和刷新操作；重命名仅允许单选。
- 增加“显示隐藏文件”开关和文件双击行为设置，目录双击始终进入目录。
- Tauri 拖放只在指针命中 SFTP 面板时生效；上传前显示服务器、目标路径、文件数、目录数、总大小和跳过项，并在会话或路径变化时拒绝执行。
- 拖放目录批次会传播冲突取消结果，不会在用户取消后继续处理后续目录。
- 状态栏显示目录项目数、已选数量、当前目录直接文件总大小和运行传输数。
- 新增路径规范化、面包屑、刷新选择协调、精确选择快照、范围选择和拖放命中测试。

验证：Windows GitHub Actions run `29395108517` 已通过前端纯状态测试、Vue/TypeScript 类型检查、Vite 生产构建、Rust 格式、单元测试和 Clippy；未执行真实服务器写入测试。

本地待测：多会话路径切换、刷新选择保留、Ctrl/Shift 多选、批量删除、右键菜单、隐藏文件、双击下载、不同 DPI 显示器上的拖放命中和上传预览，以及中文/空格/超长路径。

下一步：九个 SFTP 计划阶段已完成；后续改动根据本地验收问题或新的独立计划开展。

---

## 14. 自动化测试计划""",
    "PR9 completion record",
)
plan = replace_once(
    plan,
    "从 PR1 开始引入轻量测试能力，优先使用 Vitest。",
    "项目使用 Node.js 原生测试运行器执行前端纯状态测试，避免为当前轻量测试范围增加额外运行时依赖。",
    "frontend test implementation",
)
plan = replace_once(
    plan,
    """计划增加命令：

```json
{
  "test:frontend": "vitest run",
  "test": "npm run test:frontend && npm run test:rust"
}
```

是否在 PR1 引入 Vitest，以该 PR 实际范围和依赖评估为准；若暂不引入，必须至少把纯状态逻辑提取成可测试模块，并在后续最早 PR 补齐。""",
    """当前命令：

```json
{
  "test:frontend": "node --experimental-strip-types --test src/sftp/*.test.mjs",
  "test": "npm run test:frontend && npm run test:rust"
}
```

会话、队列和导航逻辑均已提取为纯 TypeScript 模块，并由 `.test.mjs` 覆盖关键竞态和状态转换。""",
    "frontend test commands",
)
section_start = plan.index("## 16. 当前下一步")
plan = plan[:section_start] + """## 16. 当前状态

九个 SFTP 改造阶段均已完成：

1. 会话状态隔离。
2. 相同目标路径传输互斥。
3. 文件与目录冲突保护。
4. 统一传输终态和清理。
5. 安全断点续传和任务检查点。
6. 递归传输和符号链接安全。
7. 明确目录冲突语义。
8. 后端统一传输队列、暂停和恢复。
9. SFTP 导航与批量操作完善。

后续工作不再自动追加到本计划。发现本地验收缺陷时应新建小步修复 PR；新增远程编辑、预览、端口转发等能力时应先建立独立计划并重新确认安全边界。
"""
plan_path.write_text(plan, encoding="utf-8")

readme_path = Path("README.md")
readme = readme_path.read_text(encoding="utf-8")
readme = replace_once(
    readme,
    "- 文件冲突支持覆盖、跳过和重命名；目录冲突独立支持合并、跳过、重命名和 staging 式安全替换。",
    """- 文件冲突支持覆盖、跳过和重命名；目录冲突独立支持合并、跳过、重命名和 staging 式安全替换。
- 支持面包屑与可编辑路径、每会话独立导航、刷新保留有效选择、Ctrl/Shift 多选和批量删除。
- 支持文件右键菜单、隐藏文件开关和可配置的文件双击行为。
- 文件拖放仅在 SFTP 面板范围内生效，上传前展示服务器、目标路径、文件/目录数量、总大小和跳过项。""",
    "README SFTP features",
)
readme = replace_once(readme, "9. 导航与批量操作完善（下一步）。", "9. 导航与批量操作完善（PR9 已完成）。", "README route")
readme = replace_once(
    readme,
    "- PR1～PR8 已完成可靠性改造，下一阶段为导航、批量操作和拖放作用域完善。",
    "- PR1～PR9 的 SFTP 可靠性与交互改造已完成；仍需在真实窗口和专用服务器临时目录中完成综合验收。",
    "README limits progress",
)
readme_path.write_text(readme, encoding="utf-8")

handoff_path = Path("handoff.md")
handoff = handoff_path.read_text(encoding="utf-8")
handoff = replace_once(
    handoff,
    "当前状态：PR1～PR7 已合并；PR8 后端持久传输队列、暂停和恢复已实现，并通过 Windows 全量验证，等待最终标准 CI、code review 和合并。PR8 合并后进入 PR9。",
    "当前状态：PR1～PR9 已完成并进入主线，`plan.md` 中的九阶段 SFTP 改造计划已经结束。",
    "handoff stage status",
)
handoff = replace_once(
    handoff,
    "- PR8：任务事件与快照按单调版本合并，暂停和重启恢复以真实检查点为准；服务器重连后按稳定服务器身份自动唤醒任务。",
    """- PR8：任务事件与快照按单调版本合并，暂停和重启恢复以真实检查点为准；服务器重连后按稳定服务器身份自动唤醒任务。
- PR9：面包屑和可编辑路径、刷新保留有效选择、Ctrl/Shift 多选、批量删除、右键菜单、隐藏文件和文件双击设置。
- PR9：拖放命中限制在 SFTP 面板，上传预览绑定服务器和目标目录，并展示文件/目录数、总大小及跳过项。""",
    "handoff PR9 features",
)
handoff = replace_once(
    handoff,
    "9. 拖放监听整个 Tauri 窗口，不限于 SFTP 区域，留到 PR9 处理。",
    "9. PR9 已限制拖放命中范围并增加上传预览；不同 Windows DPI 和多显示器缩放仍需本地实机验证。",
    "handoff risk 9",
)
handoff = replace_once(
    handoff,
    "PR1～PR8 已完成核心数据安全、传输可靠性、递归边界、目录事务和后端队列改造；后续按计划处理导航与批量操作。",
    "PR1～PR9 已完成核心数据安全、传输可靠性、递归边界、目录事务、后端队列和日常文件管理交互改造。",
    "handoff risk summary",
)
current_start = handoff.index("## 6. 当前任务与下一任务")
current_end = handoff.index("## 7. 关键文件", current_start)
handoff = handoff[:current_start] + """## 6. 当前任务与下一任务

当前任务：九阶段 SFTP 改造计划已经完成，没有自动排定的下一 PR。

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

自动化验证：Windows GitHub Actions run `29395108517` 已通过前端纯状态测试、类型检查、生产构建、Rust 格式、测试和 Clippy。

尚需本地实机验证：

- 不同 DPI/多显示器环境下拖放命中范围。
- 双服务器切换时的路径、选择和右键操作隔离。
- 专用临时目录中的批量删除、目录递归删除和部分失败提示。
- 拖放上传预览的数量、大小、服务器和目标路径。
- 中文、空格、超长路径及大量文件列表。
- PR1～PR8 在 `plan.md` 手工测试矩阵中列出的弱网、重启、链接和冲突场景。

后续发现缺陷时从最新 `main` 创建独立小步修复分支；新增功能应先写新的计划，不继续扩展已完成的 `plan.md`。

""" + handoff[current_end:]
handoff = replace_once(
    handoff,
    """- `src/sftp/transfer-queue-state.test.mjs`
  - PR8 极快任务和快照竞态测试。""",
    """- `src/sftp/transfer-queue-state.test.mjs`
  - PR8 极快任务和快照竞态测试。
- `src/sftp/navigation-state.ts`
  - PR9 路径面包屑、选择协调、精确快照和拖放命中纯逻辑。
- `src/sftp/navigation-state.test.mjs`
  - PR9 导航、多选、刷新选择和拖放命中测试。""",
    "handoff key navigation files",
)
handoff_path.write_text(handoff, encoding="utf-8")
