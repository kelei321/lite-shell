from pathlib import Path

path = Path("scripts/apply_pr6.py")
text = path.read_text(encoding="utf-8")
start_marker = """    text = replace_once(
        text,
        '''    transfers.cancelled.lock().await.insert(transfer_id);
"""
next_marker = """    text = replace_once(
        text,
        '''
    #[tokio::test]
"""
start = text.index(start_marker)
end = text.index(next_marker, start)
replacement = '''    cancel_start = text.index("    transfers.cancelled.lock().await.insert(transfer_id);")
    prepare_start = text.index(
        "#[tauri::command]\\npub async fn sftp_prepare_local_directory",
        cancel_start,
    )
    text = (
        text[:cancel_start]
        + "    transfers.cancel_operation(&transfer_id).await;\\n    Ok(())\\n}\\n\\n"
        + text[prepare_start:]
    )
'''
path.write_text(text[:start] + replacement + text[end:], encoding="utf-8", newline="\n")
Path("scripts/fix_apply_pr6.py").unlink()
