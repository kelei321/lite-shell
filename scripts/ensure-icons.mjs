import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

const iconPath = resolve('src-tauri/icons/icon.ico');
const iconBase64 =
  'AAABAAEAEBAAAAEAIABoBAAAFgAAACgAAAAQAAAAIAAAAAEAIAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAADrYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/////////////////////////////////////////////////rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/////////////////////////////////////////////////62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/////////////////////////////////////////////////+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/////////////////rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/////////////////62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/////////////////+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/////////////////rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/////////////////62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/////////////////+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/////////////////rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/////////////////62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==';

mkdirSync(dirname(iconPath), { recursive: true });
writeFileSync(iconPath, Buffer.from(iconBase64, 'base64'));
console.log(`wrote ${iconPath}`);
