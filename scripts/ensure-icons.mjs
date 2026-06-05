import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

const iconPath = resolve('src-tauri/icons/icon.ico');

if (!existsSync(iconPath)) {
  mkdirSync(dirname(iconPath), { recursive: true });
  writeFileSync(
    iconPath,
    Buffer.from(
      'AAABAAEAEBAAAAEAIABoBAAAFgAAACgAAAAQAAAAIAAAAAEAIAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAAAADrYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX////////////////////////////////////////////rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml////////////////////////////////////////////62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf///////////////////////////////////////////+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/////////////////62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/////////////////+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/////////////////rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/////////////////62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/////////////////+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/////////////////rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/////////////////62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/62Ml/+tjJf/rYyX/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==',
      'base64',
    ),
  );
  console.log(`created ${iconPath}`);
}
