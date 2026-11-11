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
/** Fail-closed installation of one reviewed dependency patch, with finite rollback ownership. */
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeMirrorPatchManifest } from './codemirror-view-manifest.js';
import type { CodeMirrorPatchManifest } from './codemirror-view-manifest.js';

/** Injected synchronous filesystem boundary, allowing partial-write and rollback proof. */
export interface PatchFileSystem {
  /** Read exact dependency bytes. */
  read(path: string): Buffer;
  /** Replace exact entrypoint bytes; a thrown write may already have changed the file. */
  write(path: string, bytes: Buffer): void;
}

/** Installation result includes no dependency source or user content. */
export type PatchOutcome = 'applied' | 'already-applied' | 'verified';

/** Node adapter owns no persistent resources, background work or temporary files. */
const nodeFileSystem: PatchFileSystem = {
  read: readFileSync,
  write: writeFileSync,
};

/** Hash complete bytes, without newline or encoding normalization. */
export function sha256(bytes: Buffer): string {
  return createHash('sha256').update(bytes).digest('hex');
}

/** Parse the exact pinned metadata before trusting its public identity. */
function checkPackage(bytes: Buffer, manifest: CodeMirrorPatchManifest): void {
  if (sha256(bytes) !== manifest.package.packageSha256) throw new Error('CM_PACKAGE_DRIFT');
  const value: unknown = JSON.parse(bytes.toString('utf8'));
  if (
    typeof value !== 'object' ||
    value === null ||
    !('name' in value) ||
    !('version' in value) ||
    !('license' in value) ||
    value.name !== manifest.package.name ||
    value.version !== manifest.package.version ||
    value.license !== 'MIT'
  )
    throw new Error('CM_PACKAGE_IDENTITY');
}

/** Apply ordered exact-once literals, then verify the complete resulting candidate bytes before any filesystem write. */
function candidate(
  bytes: Buffer,
  entry: CodeMirrorPatchManifest['entries'][number],
  manifest: CodeMirrorPatchManifest,
): Buffer {
  let result = bytes;
  for (const replacement of manifest.replacements)
    result = replaceOnce(result, replacement.before, replacement.after);
  if (sha256(result) !== entry.afterSha256) throw new Error('CM_CANDIDATE_DRIFT');
  return result;
}

/** Replace each literal exactly once, in order; a missing or repeated site is drift. */
function replaceOnce(bytes: Buffer, from: string, to: string): Buffer {
  const before = Buffer.from(from),
    position = bytes.indexOf(before);
  if (position < 0 || bytes.indexOf(before, position + before.length) >= 0)
    throw new Error('CM_REPLACEMENT_SITE');
  return Buffer.concat([
    bytes.subarray(0, position),
    Buffer.from(to),
    bytes.subarray(position + before.length),
  ]);
}

/** Recover sealed upstream bytes from a superseded installed generation, or reject the bytes as drift. */
function upstreamBytes(
  installed: Buffer,
  index: number,
  entry: CodeMirrorPatchManifest['entries'][number],
  manifest: CodeMirrorPatchManifest,
): Buffer | null {
  const identity = sha256(installed);
  if (identity === entry.beforeSha256) return installed;
  const generation = manifest.superseded.find(
    (candidate) => candidate.afterSha256[index] === identity,
  );
  if (!generation) return null;
  let upstream = installed;
  for (const replacement of [...generation.replacements].reverse())
    upstream = replaceOnce(upstream, replacement.after, replacement.before);
  return sha256(upstream) === entry.beforeSha256 ? upstream : null;
}

/** Preflight both complete builds, then install once; any write or verification failure rolls back every attempted write. */
export function installCodeMirrorPatch(
  packageRoot: string,
  mode: 'apply' | 'check',
  fs: PatchFileSystem = nodeFileSystem,
  manifest: CodeMirrorPatchManifest = codeMirrorPatchManifest,
): PatchOutcome {
  checkPackage(fs.read(resolve(packageRoot, 'package.json')), manifest);
  if (sha256(fs.read(resolve(packageRoot, 'LICENSE'))) !== manifest.package.licenseSha256)
    throw new Error('CM_LICENSE_DRIFT');
  const entries = manifest.entries.map((entry, index) => {
    const path = resolve(packageRoot, entry.path),
      original = fs.read(path);
    if (sha256(original) === entry.afterSha256)
      return { entry, path, original, upstream: original, applied: true };
    const upstream = upstreamBytes(original, index, entry, manifest);
    if (!upstream) throw new Error('CM_ENTRY_DRIFT');
    return { entry, path, original, upstream, applied: false };
  });
  if (entries.every((entry) => entry.applied))
    return mode === 'check' ? 'verified' : 'already-applied';
  if (entries.some((entry) => entry.applied)) throw new Error('CM_MIXED_INSTALLATION');
  const prepared = entries.map((entry) => ({
    ...entry,
    replacement: candidate(entry.upstream, entry.entry, manifest),
  }));
  if (mode === 'check') throw new Error('CM_PATCH_MISSING');
  const attempted: typeof prepared = [];
  try {
    for (const entry of prepared) {
      attempted.push(entry);
      fs.write(entry.path, entry.replacement);
    }
    for (const entry of prepared)
      if (sha256(fs.read(entry.path)) !== entry.entry.afterSha256)
        throw new Error('CM_WRITE_VERIFICATION');
  } catch (primary: unknown) {
    const failures: unknown[] = [primary];
    for (const entry of attempted.reverse()) {
      try {
        fs.write(entry.path, entry.original);
        if (!fs.read(entry.path).equals(entry.original))
          throw new Error('CM_ROLLBACK_VERIFICATION');
      } catch (rollback: unknown) {
        failures.push(rollback);
      }
    }
    if (failures.length > 1)
      throw new AggregateError(failures, 'CM_INSTALL_AND_ROLLBACK_FAILED', {
        cause: primary,
      });
    throw primary;
  }
  return 'applied';
}
