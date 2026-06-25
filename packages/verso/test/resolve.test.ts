import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

import {
  binaryName as exportedBinaryName,
  packageForPlatform as exportedPackageForPlatform,
} from '@univerkit/verso/resolve';
import {
  binaryName,
  packageForPlatform,
  resolvePlatformBinary,
} from '../src/resolve.js';

describe('packageForPlatform', () => {
  it('maps supported platform and architecture pairs to platform packages', () => {
    assert.equal(
      packageForPlatform('darwin', 'arm64'),
      '@univerkit/verso-darwin-arm64',
    );
    assert.equal(
      packageForPlatform('darwin', 'x64'),
      '@univerkit/verso-darwin-x64',
    );
    assert.equal(
      packageForPlatform('linux', 'arm64'),
      '@univerkit/verso-linux-arm64',
    );
    assert.equal(
      packageForPlatform('linux', 'x64'),
      '@univerkit/verso-linux-x64',
    );
    assert.equal(
      packageForPlatform('win32', 'x64'),
      '@univerkit/verso-win32-x64',
    );
  });

  it('throws a clear error for unsupported platforms', () => {
    assert.throws(
      () => packageForPlatform('freebsd', 'x64'),
      /Unsupported platform: freebsd x64/,
    );
  });

  it('throws a clear error for unsupported architectures', () => {
    assert.throws(
      () => packageForPlatform('linux', 'arm'),
      /Unsupported platform: linux arm/,
    );
  });
});

describe('binaryName', () => {
  it('uses the Windows executable suffix', () => {
    assert.equal(binaryName('win32'), 'verso.exe');
  });

  it('uses the extensionless executable name for non-Windows platforms', () => {
    assert.equal(binaryName('darwin'), 'verso');
    assert.equal(binaryName('linux'), 'verso');
  });
});

describe('resolvePlatformBinary', () => {
  it('composes the platform package and binary name helpers', () => {
    assert.deepEqual(resolvePlatformBinary('win32', 'x64'), {
      packageName: '@univerkit/verso-win32-x64',
      binaryName: 'verso.exe',
    });
  });
});

describe('build output', () => {
  it('creates the bin entrypoint', () => {
    const binPath = join(process.cwd(), 'bin', 'verso.js');

    assert.equal(existsSync(binPath), true);
    assert.equal(readFileSync(binPath, 'utf8').startsWith('#!/usr/bin/env node\n'), true);
    if (process.platform !== 'win32') {
      assert.equal((statSync(binPath).mode & 0o111) !== 0, true);
    }
  });
});

describe('package boundary exports', () => {
  it('exports resolver helpers from @univerkit/verso/resolve', () => {
    assert.equal(
      exportedPackageForPlatform('linux', 'x64'),
      '@univerkit/verso-linux-x64',
    );
    assert.equal(exportedBinaryName('win32'), 'verso.exe');
  });
});
