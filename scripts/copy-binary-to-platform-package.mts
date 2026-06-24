import { access, chmod, copyFile, mkdir } from 'node:fs/promises';
import { basename, resolve } from 'node:path';
import { constants } from 'node:fs';

function fail(message: string): never {
  console.error(message);
  process.exit(1);
}

const [, , packageDirArg, binaryPathArg] = process.argv;

if (packageDirArg === undefined || binaryPathArg === undefined) {
  fail('Usage: node scripts/copy-binary-to-platform-package.mts <platform-package-dir> <binary-path>');
}

const packageDir = resolve(packageDirArg);
const binaryPath = resolve(binaryPathArg);

try {
  await access(binaryPath, constants.F_OK);
} catch {
  fail(`Source binary does not exist: ${binaryPath}`);
}

const binaryName = basename(binaryPath).endsWith('.exe') ? 'verso.exe' : 'verso';
const binDir = resolve(packageDir, 'bin');
const destination = resolve(binDir, binaryName);

await mkdir(binDir, { recursive: true });
await copyFile(binaryPath, destination);
await chmod(destination, 0o755);

console.log(`Copied binary to ${destination}`);
