from pathlib import Path


def update(path: str, replacements: list[tuple[str, str, str]]) -> None:
    file_path = Path(path)
    text = file_path.read_text(encoding="utf-8")
    for old, new, label in replacements:
        count = text.count(old)
        if count != 1:
            raise RuntimeError(f"{path} {label}: expected one match, found {count}")
        text = text.replace(old, new, 1)
    file_path.write_text(text, encoding="utf-8", newline="\n")


update(
    "plan.md",
    [
        (
            "状态：实施进行中，PR1～PR6 已完成，PR7 已实现并待验证",
            "状态：实施进行中，PR1～PR6 已完成，PR7 已通过验证并等待合并",
            "summary status",
        ),
        (
            "验证：等待 GitHub Actions；未执行真实服务器写入测试。\n\n本地待测：本地目录替换、远程目录 merge/replace、远程 rename 不支持、最终 rename 切换中断与 staging/backup 残留恢复。",
            "验证：GitHub Actions CI run `29311102314` 已通过 Frontend 类型检查/构建、Rust 格式、单元测试和 Clippy；未执行真实服务器写入测试。\n\n本地待测：本地目录替换、远程目录 merge/replace、远程 rename 不支持、最终 rename 切换中断与 staging/backup 残留恢复。",
            "PR7 validation",
        ),
    ],
)

update(
    "README.md",
    [
        (
            "- PR1～PR6 已完成；PR7 已实现目录冲突分离和 staging 式替换，正在等待 CI、code review 和合并。",
            "- PR1～PR6 已完成；PR7 已实现目录冲突分离和 staging 式替换，CI 与 code review 已通过，等待合并。",
            "progress",
        ),
    ],
)

update(
    "handoff.md",
    [
        (
            "当前状态：PR1～PR6 已合并；PR7 目录冲突语义已实现，正在等待 CI、code review 和合并。PR7 合并后进入 PR8。",
            "当前状态：PR1～PR6 已合并；PR7 目录冲突语义已实现，CI 与 code review 已通过，等待合并。PR7 合并后进入 PR8。",
            "current status",
        ),
        (
            "当前任务：PR7 明确目录冲突语义，分支 `feat/sftp-directory-conflict-strategies`，状态为待验证。",
            "当前任务：PR7 明确目录冲突语义，分支 `feat/sftp-directory-conflict-strategies`，状态为待验证；自动化验证与 code review 已通过。",
            "current task",
        ),
    ],
)

Path("scripts/finalize_pr9_docs.py").unlink()
Path(".github/workflows/finalize-pr9-docs.yml").unlink()
