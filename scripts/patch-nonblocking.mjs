import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const file = resolve('src-tauri', 'src', 'ssh.rs');
const marker = '    session.set_blocking(false);';
const anchor = '    let handle = Arc::new(SshHandle {';
let lines = readFileSync(file, 'utf8').split(/\r?\n/);
const first = lines.indexOf(marker);

if (first >= 0) {
  lines.splice(first, 1);
}

const anchorIndex = lines.indexOf(anchor);
const existsBeforeAnchor = anchorIndex > 0 && lines[anchorIndex - 2] === marker;

if (anchorIndex >= 0 && !existsBeforeAnchor) {
  lines.splice(anchorIndex, 0, marker, '');
}

writeFileSync(file, `${lines.join('\n')}\n`);
