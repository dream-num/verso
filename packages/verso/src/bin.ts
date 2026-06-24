import { spawnSync } from 'node:child_process';
import { chmodSync } from 'node:fs';
import { createRequire } from 'node:module';

import { type PlatformBinary, resolvePlatformBinary } from './resolve.js';

const require = createRequire(import.meta.url);

function resolveInstalledBinaryPath({ packageName, binaryName }: PlatformBinary): string {
  try {
    return require.resolve(`${packageName}/bin/${binaryName}`);
  } catch (cause) {
    throw new Error(
      `Could not find Verso platform binary ${packageName}/bin/${binaryName}. ` +
        `The optional dependency ${packageName} may not be installed for this platform.`,
      { cause },
    );
  }
}

function normalizeCliArgs(args: string[]): string[] {
  return args[0] === '--' ? args.slice(1) : args;
}

function ensureExecutable(binaryPath: string): void {
  if (process.platform === 'win32') {
    return;
  }

  chmodSync(binaryPath, 0o755);
}

function main(): never {
  const platformBinary = resolvePlatformBinary();
  const binaryPath = resolveInstalledBinaryPath(platformBinary);

  ensureExecutable(binaryPath);

  const result = spawnSync(binaryPath, normalizeCliArgs(process.argv.slice(2)), {
    stdio: 'inherit',
  });

  if (result.error !== undefined) {
    throw new Error(`Failed to launch Verso binary at ${binaryPath}.`, {
      cause: result.error,
    });
  }

  if (result.signal !== null) {
    process.kill(process.pid, result.signal);
  }

  process.exit(result.status ?? 1);
}

try {
  main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  console.error(message);
  if (error instanceof Error && error.cause instanceof Error) {
    console.error(error.cause.message);
  }
  process.exit(1);
}
