//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
/** Safeguard exact build-output comparison and active runtime-directory preservation. */
import { afterEach, beforeEach } from '@jest/globals';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fork, type ChildProcess } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { synchronizeWebArtifacts } from '../../../scripts/WebArtifactSynchronization.js';

let root: string;
let generated: string;
let runtime: string;

beforeEach(() => {
  root = mkdtempSync(
    path.join(path.resolve('D:/devartifacts/SugarCubes/output-saving'), 'artifact-proof-'),
  );
  generated = path.join(root, 'generated');
  runtime = path.join(root, 'runtime');
  mkdirSync(generated);
  mkdirSync(runtime);
});

afterEach(() => {
  rmSync(root, { recursive: true, force: true });
});

test('exact verification rejects missing, stale and unexpected files without changing them', () => {
  writeFileSync(path.join(generated, 'same.js'), 'same');
  writeFileSync(path.join(runtime, 'same.js'), 'same');
  writeFileSync(path.join(generated, 'stale.js'), 'new');
  writeFileSync(path.join(runtime, 'stale.js'), 'old');
  writeFileSync(path.join(generated, 'missing.js'), 'missing');
  writeFileSync(path.join(runtime, 'unexpected.js'), 'extra');
  expect(() => synchronizeWebArtifacts(generated, runtime, true)).toThrow(/missing: missing.js/);
  expect(() => synchronizeWebArtifacts(generated, runtime, true)).toThrow(/stale: stale.js/);
  expect(() => synchronizeWebArtifacts(generated, runtime, true)).toThrow(
    /unexpected: unexpected.js/,
  );
  expect(readFileSync(path.join(runtime, 'stale.js'), 'utf8')).toBe('old');
  expect(existsSync(path.join(runtime, 'missing.js'))).toBe(false);
  expect(existsSync(path.join(runtime, 'unexpected.js'))).toBe(true);
});

test('synchronization adds generated files, replaces stale bytes and removes obsolete artifacts', () => {
  mkdirSync(path.join(generated, 'module'));
  writeFileSync(path.join(generated, 'module/new.js'), 'new');
  writeFileSync(path.join(generated, 'stale.js'), 'updated');
  writeFileSync(path.join(runtime, 'stale.js'), 'old');
  writeFileSync(path.join(runtime, 'obsolete.js'), 'obsolete');
  synchronizeWebArtifacts(generated, runtime, false);
  expect(readFileSync(path.join(runtime, 'module/new.js'), 'utf8')).toBe('new');
  expect(readFileSync(path.join(runtime, 'stale.js'), 'utf8')).toBe('updated');
  expect(existsSync(path.join(runtime, 'obsolete.js'))).toBe(false);
  expect(() => synchronizeWebArtifacts(generated, runtime, true)).not.toThrow();
});

test('unchanged runtime artifacts retain their identity and modification time', () => {
  writeFileSync(path.join(generated, 'unchanged.js'), 'same');
  writeFileSync(path.join(runtime, 'unchanged.js'), 'same');
  const before = statSync(path.join(runtime, 'unchanged.js'));
  synchronizeWebArtifacts(generated, runtime, false);
  const after = statSync(path.join(runtime, 'unchanged.js'));
  expect(after.ino).toBe(before.ino);
  expect(after.mtimeMs).toBe(before.mtimeMs);
});

test('generated files and directories can replace obsolete artifact kinds beneath the retained root', () => {
  mkdirSync(path.join(generated, 'became-directory'));
  writeFileSync(path.join(generated, 'became-directory/module.js'), 'module');
  writeFileSync(path.join(runtime, 'became-directory'), 'old file');
  writeFileSync(path.join(generated, 'became-file'), 'new file');
  mkdirSync(path.join(runtime, 'became-file/nested'), { recursive: true });
  writeFileSync(path.join(runtime, 'became-file/nested/old.js'), 'old');
  synchronizeWebArtifacts(generated, runtime, false);
  expect(readFileSync(path.join(runtime, 'became-directory/module.js'), 'utf8')).toBe('module');
  expect(readFileSync(path.join(runtime, 'became-file'), 'utf8')).toBe('new file');
});

test('overlapping roots are rejected before removing or copying any artifact', () => {
  writeFileSync(path.join(generated, 'retained.js'), 'retained');
  expect(() => synchronizeWebArtifacts(generated, generated, false)).toThrow('separate trees');
  expect(() => synchronizeWebArtifacts(root, generated, false)).toThrow('separate trees');
  expect(readFileSync(path.join(generated, 'retained.js'), 'utf8')).toBe('retained');
});

/** Observe one IPC boundary with a bounded lifetime and no test-order dependence. */
function childDirectory(child: ChildProcess): Promise<string> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      cleanup();
      reject(new Error('Directory holder did not respond.'));
    }, 5000);
    /** Release listeners on success, failure or timeout. */
    const cleanup = () => {
      clearTimeout(timer);
      child.removeListener('message', receive);
      child.removeListener('error', failed);
    };
    /** Validate the external child response before reading a filesystem assertion value. */
    const receive = (message: unknown) => {
      cleanup();
      if (
        typeof message === 'object' &&
        message !== null &&
        'cwd' in message &&
        typeof message.cwd === 'string'
      )
        resolve(message.cwd);
      else reject(new Error('Directory holder returned an invalid response.'));
    };
    /** Preserve child-startup failure context. */
    const failed = (error: Error) => {
      cleanup();
      reject(error);
    };
    child.on('message', receive);
    child.on('error', failed);
  });
}

test('rebuilding preserves a runtime directory held by an active host process', async () => {
  const child = fork(path.resolve('tests/governance/support/HoldArtifactDirectory.ts'), [], {
    cwd: runtime,
    execArgv: ['--import', pathToFileURL(path.resolve('node_modules/tsx/dist/loader.mjs')).href],
    stdio: ['ignore', 'ignore', 'ignore', 'ipc'],
  });
  const exited = new Promise<void>((resolve) => {
    child.once('exit', () => resolve());
  });
  try {
    expect(await childDirectory(child)).toBe(runtime);
    writeFileSync(path.join(generated, 'new.js'), 'new');
    writeFileSync(path.join(runtime, 'obsolete.js'), 'old');
    synchronizeWebArtifacts(generated, runtime, false);
    const directory = childDirectory(child);
    child.send('cwd');
    expect(await directory).toBe(runtime);
    expect(readFileSync(path.join(runtime, 'new.js'), 'utf8')).toBe('new');
    expect(existsSync(path.join(runtime, 'obsolete.js'))).toBe(false);
  } finally {
    child.kill();
    await exited;
  }
}, 15000);
