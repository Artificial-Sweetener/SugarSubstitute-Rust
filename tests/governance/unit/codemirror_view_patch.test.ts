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
/** Verify the pinned selection and viewport patch is reproducible, fail closed and atomic. */
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeMirrorPatchManifest } from '../../../scripts/dependencies/codemirror-view-manifest.js';
import {
  installCodeMirrorPatch,
  sha256,
  type PatchFileSystem,
} from '../../../scripts/dependencies/codemirror-view-patch.js';

/** Reconstruct sealed upstream bytes in memory without modifying the installed dependency. */
function fixture() {
  const root = resolve('D:/SugarCubes/virtual-codemirror');
  const installed = resolve('node_modules/@codemirror/view');
  const files = new Map<string, Buffer>();
  for (const name of ['package.json', 'LICENSE'])
    files.set(resolve(root, name), readFileSync(resolve(installed, name)));
  for (const entry of codeMirrorPatchManifest.entries) {
    const bytes = readFileSync(resolve(installed, entry.path));
    expect(sha256(bytes)).toBe(entry.afterSha256);
    let text = bytes.toString('utf8');
    for (const replacement of [...codeMirrorPatchManifest.replacements].reverse())
      text = text.replace(replacement.after, replacement.before);
    const upstream = Buffer.from(text);
    expect(sha256(upstream)).toBe(entry.beforeSha256);
    files.set(resolve(root, entry.path), upstream);
  }
  const writes: string[] = [];
  const fs: PatchFileSystem = {
    read(path) {
      const bytes = files.get(path);
      if (!bytes) throw new Error('Missing fixture dependency');
      return Buffer.from(bytes);
    },
    write(path, bytes) {
      writes.push(path);
      files.set(path, Buffer.from(bytes));
    },
  };
  return { root, files, writes, fs };
}

test('applies all reviewed edits once and verifies both complete distribution builds', () => {
  const f = fixture();
  expect(() => installCodeMirrorPatch(f.root, 'check', f.fs)).toThrow('CM_PATCH_MISSING');
  expect(f.writes).toEqual([]);
  expect(installCodeMirrorPatch(f.root, 'apply', f.fs)).toBe('applied');
  expect(f.writes).toHaveLength(2);
  expect(installCodeMirrorPatch(f.root, 'check', f.fs)).toBe('verified');
  expect(installCodeMirrorPatch(f.root, 'apply', f.fs)).toBe('already-applied');
  expect(f.writes).toHaveLength(2);
  for (const entry of codeMirrorPatchManifest.entries)
    expect(sha256(f.fs.read(resolve(f.root, entry.path)))).toBe(entry.afterSha256);
});

test('rejects unreviewed dependency bytes before writing', () => {
  const f = fixture();
  const path = resolve(f.root, codeMirrorPatchManifest.entries[0].path);
  f.files.set(path, Buffer.concat([f.fs.read(path), Buffer.from('\n')]));
  expect(() => installCodeMirrorPatch(f.root, 'apply', f.fs)).toThrow('CM_ENTRY_DRIFT');
  expect(f.writes).toEqual([]);
});

test('restores both original entrypoints after a partial second write fails', () => {
  const f = fixture();
  const fs: PatchFileSystem = {
    read: f.fs.read,
    write(path, bytes) {
      f.fs.write(path, bytes);
      if (f.writes.length === 2) throw new Error('Partial write');
    },
  };
  expect(() => installCodeMirrorPatch(f.root, 'apply', fs)).toThrow('Partial write');
  for (const entry of codeMirrorPatchManifest.entries)
    expect(sha256(f.fs.read(resolve(f.root, entry.path)))).toBe(entry.beforeSha256);
});
