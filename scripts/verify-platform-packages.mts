import { access } from 'node:fs/promises';
import { constants } from 'node:fs';
import { resolve } from 'node:path';

const expectedBinaries = [
  'packages/verso-darwin-arm64/bin/verso',
  'packages/verso-darwin-x64/bin/verso',
  'packages/verso-linux-arm64/bin/verso',
  'packages/verso-linux-x64/bin/verso',
  'packages/verso-win32-x64/bin/verso.exe',
];

const missingFiles: string[] = [];

for (const relativePath of expectedBinaries) {
  const absolutePath = resolve(relativePath);

  try {
    await access(absolutePath, constants.F_OK);
  } catch {
    missingFiles.push(relativePath);
  }
}

if (missingFiles.length > 0) {
  console.error('Missing platform package binaries:');
  for (const missingFile of missingFiles) {
    console.error(`- ${missingFile}`);
  }
  process.exit(1);
}

console.log('All platform package binaries are present.');
