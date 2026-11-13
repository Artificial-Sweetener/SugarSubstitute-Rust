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
/** Closed host result adaptation delegates all wildcard metadata semantics to the existing catalog owner. */
import { prepareWildcardCatalog } from '../wildcards/catalog.js';
import type { WildcardCatalogInput } from '../wildcards/contracts.js';
import { managedWildcardFile } from './paths.js';
import type {
  ManagedWildcardSnapshot,
  ManagedWildcardStorageFailure,
  ManagedWildcardReadResult,
  ManagedWildcardChangeResult,
  ManagedWildcardStorage,
  ManagedWildcardFolderAdapter,
} from './contracts.js';

/** Require a closed plain reply; invalid external contracts remain observable invariant failures. */
function record(value: unknown, keys: readonly string[]): value is Record<string, unknown> {
  return (
    value !== null &&
    typeof value === 'object' &&
    !Array.isArray(value) &&
    Object.keys(value).sort().join() === [...keys].sort().join()
  );
}
/** Require bounded nonempty opaque revisions without decoding a host's persistence format. */
export function managedWildcardRevision(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0 && value.length <= 256;
}
/** Validate expected storage failure codes independently of unexpected thrown causes. */
export function managedWildcardFailure(value: unknown): ManagedWildcardStorageFailure | null {
  return record(value, ['status']) &&
    typeof value.status === 'string' &&
    ['unavailable', 'denied', 'missing', 'conflict', 'stale'].includes(value.status)
    ? Object.freeze({
        status: value.status as ManagedWildcardStorageFailure['status'],
      })
    : null;
}
/** Capture callable adapter methods once, retaining their host receiver and rejecting malformed capabilities. */
export function captureManagedWildcardStorage(
  value: ManagedWildcardStorage,
): ManagedWildcardStorage {
  if (
    !value ||
    ['list', 'read', 'write', 'rename', 'delete', 'invalidate'].some(
      (key) => typeof value[key as keyof ManagedWildcardStorage] !== 'function',
    )
  )
    throw new TypeError('Invalid managed wildcard storage adapter');
  return Object.freeze({
    list: value.list.bind(value),
    read: value.read.bind(value),
    write: value.write.bind(value),
    rename: value.rename.bind(value),
    delete: value.delete.bind(value),
    invalidate: value.invalidate.bind(value),
  });
}
/** Capture the optional host folder capability independently of managed filesystem identities. */
export function captureManagedWildcardFolder(
  value: ManagedWildcardFolderAdapter | undefined,
): ManagedWildcardFolderAdapter | undefined {
  if (value === undefined) return undefined;
  if (!value || typeof value.open !== 'function')
    throw new TypeError('Invalid managed wildcard folder adapter');
  return Object.freeze({ open: value.open.bind(value) });
}
/** Capture one current list and build its sole immutable existing catalog, without retaining host arrays/maps. */
export function captureManagedWildcardList(
  value: unknown,
): Extract<ManagedWildcardSnapshot, { status: 'ready' }> | ManagedWildcardStorageFailure {
  const failure = managedWildcardFailure(value);
  if (failure) return failure;
  if (
    !record(value, ['status', 'revision', 'paths', 'catalog']) ||
    value.status !== 'ready' ||
    !managedWildcardRevision(value.revision) ||
    !Array.isArray(value.paths) ||
    value.paths.length > 10000
  )
    throw new TypeError('Invalid managed wildcard list result');
  const files = [],
    seen = new Set<string>();
  let units = 0;
  for (const path of value.paths as readonly unknown[]) {
    const file = managedWildcardFile(path);
    if (!file || file.path !== path || seen.has(file.path) || (units += file.path.length) > 8000000)
      throw new TypeError('Invalid managed wildcard file entry');
    seen.add(file.path);
    files.push(file);
  }
  const catalog = prepareWildcardCatalog(value.catalog as WildcardCatalogInput);
  if (catalog.status !== 'ready' || catalog.revision !== value.revision)
    throw new TypeError('Invalid managed wildcard catalog revision');
  return Object.freeze({
    status: 'ready',
    revision: value.revision,
    files: Object.freeze(files),
    catalog,
  });
}
/** Validate exact decoded source and revision; no universal-newline normalization enters the library service. */
export function captureManagedWildcardRead(value: unknown): ManagedWildcardReadResult {
  const failure = managedWildcardFailure(value);
  if (failure) return failure;
  if (
    !record(value, ['status', 'revision', 'text']) ||
    value.status !== 'read' ||
    !managedWildcardRevision(value.revision) ||
    typeof value.text !== 'string' ||
    value.text.length > 8000000
  )
    throw new TypeError('Invalid managed wildcard read result');
  return Object.freeze({
    status: 'read',
    revision: value.revision,
    text: value.text,
  });
}
/** Validate confirmed persistence before invalidating or publishing newly prepared metadata. */
export function captureManagedWildcardChange(value: unknown): ManagedWildcardChangeResult {
  const failure = managedWildcardFailure(value);
  if (failure) return failure;
  if (
    !record(value, ['status', 'revision']) ||
    value.status !== 'changed' ||
    !managedWildcardRevision(value.revision)
  )
    throw new TypeError('Invalid managed wildcard change result');
  return Object.freeze({ status: 'changed', revision: value.revision });
}
/** Validate folder outcomes without accepting returned paths or arbitrary metadata. */
export function captureManagedWildcardFolderResult(value: unknown): {
  readonly status: 'opened' | 'unavailable' | 'denied';
} {
  if (
    !record(value, ['status']) ||
    typeof value.status !== 'string' ||
    !['opened', 'unavailable', 'denied'].includes(value.status)
  )
    throw new TypeError('Invalid managed wildcard folder result');
  return Object.freeze({
    status: value.status as 'opened' | 'unavailable' | 'denied',
  });
}
