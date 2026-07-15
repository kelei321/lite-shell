from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if text.count(old) != 1:
        raise RuntimeError(f"{label}: expected one match, found {text.count(old)}")
    return text.replace(old, new, 1)


plan_path = Path("plan.md")
plan = plan_path.read_text(encoding="utf-8")
plan = replace_once(
    plan,
    "状态：原九阶段改造已完成；双栏文件管理 PR1 已实现，等待 CI、合并和本地验证",
    "状态：原九阶段改造已完成；双栏文件管理 PR1 已通过 CI 与 code review，等待 squash 合并和本地验证",
    "plan header status",
)
plan = replace_once(
    plan,
    "当前状态：PR1 已实现，等待 CI、squash 合并和本地验证；PR2 尚未开始。",
    "当前状态：PR1 已通过 CI 与 code review，等待 squash 合并；合并后进入本地验证门禁，PR2 尚未开始。",
    "plan PR1 status",
)
plan = replace_once(
    plan,
    "- 新增目录树状态、请求竞态、刷新复用、祖先展开和会话清理测试。\n\n安全边界：",
    "- 新增目录树状态、请求竞态、刷新复用、祖先展开和会话清理测试。\n\n完成记录：\n\n- PR：#12 `feat: add lazy SFTP directory tree`。\n- 最终验证：GitHub Actions run `29402040972`，Frontend 类型检查/构建和 Rust 格式/测试/Clippy 全部通过。\n- Code review 修复：根路径会主动懒加载；已展开但未加载的节点首次点击直接读取；刷新节点会保持展开；目录缩进改为 Vue 计算像素，避免依赖 WebView2 对 CSS 乘法表达式的支持。\n- 尚未执行真实服务器写入操作；本 PR 新增后端命令为只读目录枚举。\n\n安全边界：",
    "plan completion record",
)
plan_path.write_text(plan, encoding="utf-8")

handoff_path = Path("handoff.md")
handoff = handoff_path.read_text(encoding="utf-8")
handoff = replace_once(
    handoff,
    "当前任务：双栏文件管理 PR1“远程目录树与双栏布局”，分支 `feat/sftp-directory-tree`。实现完成并通过 CI、code review、squash 合并后，必须停在本地验证门禁；用户确认目录树与多会话同步无误后才开始 PR2。",
    "当前任务：双栏文件管理 PR1“远程目录树与双栏布局”，PR #12 已通过 CI 与 code review，等待 squash 合并。合并后必须停在本地验证门禁；用户确认目录树与多会话同步无误后才开始 PR2。",
    "handoff current task",
)
handoff = replace_once(
    handoff,
    "PR1 不改变任何远程写入或传输协议行为。合并后不要自动开始 PR2，先等待用户给出本地验证结果。",
    "PR1 不改变任何远程写入或传输协议行为。GitHub Actions run `29402040972` 已通过 Frontend 类型检查/构建和 Rust 格式/测试/Clippy。合并后不要自动开始 PR2，先等待用户给出本地验证结果。",
    "handoff validation status",
)
handoff_path.write_text(handoff, encoding="utf-8")
