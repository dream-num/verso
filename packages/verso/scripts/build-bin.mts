import { chmod, mkdir, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const packageRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const binPath = join(packageRoot, 'bin', 'verso.js');
const binContents = "#!/usr/bin/env node\nimport '../dist/bin.js';\n";

await mkdir(dirname(binPath), { recursive: true });
await writeFile(binPath, binContents, { mode: 0o755 });
await chmod(binPath, 0o755);
