from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{label}: expected 1 match, found {count}")
    return text.replace(old, new, 1)


app_path = Path("src/App.vue")
app = app_path.read_text(encoding="utf-8")

app = replace_once(
    app,
    '''  reconcileSelection,
  updateSelectionPaths,''',
    '''  reconcileSelection,
  selectionsMatchSnapshot,
  updateSelectionPaths,''',
    "selection snapshot import",
)

app = replace_once(
    app,
    '''  allowDirectoryAll = false,
) {
  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session?.connected) return;''',
    '''  allowDirectoryAll = false,
): Promise<boolean> {
  const session = sessions.value.find((item) => item.id === sessionId);
  if (!session?.connected) return false;''',
    "directory upload result type",
)
app = replace_once(app, '''      return;
    }
    let directoryStrategy: DirectoryConflictStrategy = "merge";''', '''      return false;
    }
    let directoryStrategy: DirectoryConflictStrategy = "merge";''', "invalid root return")
app = replace_once(
    app,
    '''      const choice = await chooseDirectoryConflict(manifest.rootName, conflicts, allowDirectoryAll);
      if (choice === "cancel" || choice === "skip") return;
      directoryStrategy = choice;''',
    '''      const choice = await chooseDirectoryConflict(manifest.rootName, conflicts, allowDirectoryAll);
      if (choice === "cancel") return false;
      if (choice === "skip") return true;
      directoryStrategy = choice;''',
    "root directory conflict result",
)
app = replace_once(app, '''    if (prepared.skipped) return;

    for (const directory of manifest.directories) {''', '''    if (prepared.skipped) return true;

    for (const directory of manifest.directories) {''', "prepared skip result")
app = replace_once(
    app,
    '''          prepared = undefined;
          return;
        }
        conflictStrategy = choice;''',
    '''          prepared = undefined;
          return false;
        }
        conflictStrategy = choice;''',
    "file conflict cancellation result",
)
app = replace_once(
    app,
    '''    prepared = undefined;
    await loadDirectory(sessionId, state.path, false);
  } catch (error) {''',
    '''    prepared = undefined;
    await loadDirectory(sessionId, state.path, false);
    return true;
  } catch (error) {''',
    "directory upload success result",
)
app = replace_once(
    app,
    '''        state.error = `${describeCommandError(error)}；自动恢复原目录失败：${describeCommandError(rollbackError)}`;
        return;
      }
    }
    state.error = describeCommandError(error);
  }
}''',
    '''        state.error = `${describeCommandError(error)}；自动恢复原目录失败：${describeCommandError(rollbackError)}`;
        return false;
      }
    }
    state.error = describeCommandError(error);
    return false;
  }
}''',
    "directory upload failure result",
)

app = replace_once(
    app,
    '''  const currentPaths = new Set(state.selectedEntries.map((entry) => entry.path));
  if (activeSessionId.value !== sessionId || selected.some((entry) => !currentPaths.has(entry.path))) {''',
    '''  const currentPaths = state.selectedEntries.map((entry) => entry.path);
  const selectedPaths = selected.map((entry) => entry.path);
  if (activeSessionId.value !== sessionId || !selectionsMatchSnapshot(currentPaths, selectedPaths)) {''',
    "exact destructive selection validation",
)

app = replace_once(app, '''        directoryCount += Math.max(1, manifest.directoryCount);''', '''        directoryCount += manifest.directoryCount + 1;''', "drop root directory count")
app = replace_once(
    app,
    '''      await uploadDirectoryPath(
        directory.path,
        directory.manifest,
        preview.sessionId,
        conflicts,
        preview.directories.length > 1,
      );
      if (state.error) return;''',
    '''      const shouldContinue = await uploadDirectoryPath(
        directory.path,
        directory.manifest,
        preview.sessionId,
        conflicts,
        preview.directories.length > 1,
      );
      if (!shouldContinue || state.error) return;''',
    "drop cancellation propagation",
)
app = replace_once(app, '''<label class="sftp-option"><input v-model="showHiddenFiles" type="checkbox" />隐藏文件</label>''', '''<label class="sftp-option"><input v-model="showHiddenFiles" type="checkbox" />显示隐藏文件</label>''', "hidden file label")
app_path.write_text(app, encoding="utf-8")

styles_path = Path("src/styles.css")
styles = styles_path.read_text(encoding="utf-8")
styles = replace_once(
    styles,
    '''.file-toolbar { min-width: 0; height: 49px; display: flex; align-items: center; gap: 8px; padding: 0 12px; border-bottom: 1px solid var(--line); }''',
    '''.file-toolbar { min-width: 0; height: 49px; display: flex; align-items: center; gap: 8px; padding: 0 12px; border-bottom: 1px solid var(--line); overflow-x: auto; scrollbar-width: thin; }''',
    "toolbar overflow",
)
styles = replace_once(
    styles,
    '''.path-field { min-width: 150px; height: 34px; flex: 1; display: flex; align-items: center; padding-left: 12px; background: #132732; border: 1px solid #304753; border-radius: 3px; }''',
    '''.path-field { min-width: 260px; height: 34px; flex: 1 0 260px; display: flex; align-items: center; padding-left: 12px; background: #132732; border: 1px solid #304753; border-radius: 3px; }''',
    "path field width",
)
styles = replace_once(
    styles,
    '''.sftp-option { height: 32px; display: inline-flex; align-items: center; gap: 5px; color: #8fa3ad; white-space: nowrap; font-size: 10px; }''',
    '''.sftp-option { height: 32px; display: inline-flex; align-items: center; gap: 5px; flex: none; color: #8fa3ad; white-space: nowrap; font-size: 10px; }''',
    "option no shrink",
)
styles = replace_once(
    styles,
    '''.toolbar-button { height: 34px; display: inline-flex; align-items: center; gap: 6px; padding: 0 13px;''',
    '''.toolbar-button { height: 34px; display: inline-flex; align-items: center; gap: 6px; flex: none; padding: 0 13px;''',
    "button no shrink",
)
styles_path.write_text(styles, encoding="utf-8")
