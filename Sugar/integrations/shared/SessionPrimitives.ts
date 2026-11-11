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
/** Validate portable session identifiers and paths without granting filesystem authority. */
import type { SessionFile, SessionScope } from '../contracts/SessionProtocol.generated.js';

/** Maximum combined UTF-8 document bytes, including current and saved baseline graphs. */
export const SESSION_DOCUMENT_BYTES = 33554432;

/** Count UTF-8 bytes, rejecting unpaired UTF-16 surrogates on the browser boundary. */
export function utf8Bytes(value: string): number {
  let bytes = 0;
  for (const character of value) {
    const point = character.codePointAt(0) ?? 0;
    if (point >= 0xd800 && point <= 0xdfff) return Infinity;
    bytes += point < 0x80 ? 1 : point < 0x800 ? 2 : point < 0x10000 ? 3 : 4;
  }
  return bytes;
}

/** Reject nonfinite numbers and unpaired Unicode before JSON serialization can erase them. */
export function sessionJsonScalars(value: unknown): boolean {
  if (typeof value === 'string') return Number.isFinite(utf8Bytes(value));
  if (typeof value === 'number') return Number.isFinite(value);
  if (Array.isArray(value)) return value.every((item: unknown) => sessionJsonScalars(item));
  if (value && typeof value === 'object')
    return Object.entries(value).every(
      ([key, item]: [string, unknown]) =>
        Number.isFinite(utf8Bytes(key)) && sessionJsonScalars(item),
    );
  return value === null || typeof value === 'boolean';
}

/** Admit canonical non-nil UUIDs, independent of user-visible names and runtime keys. */
export function sessionUuid(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(value);
}

/** Admit lowercase SHA-256 hex without an algorithm prefix. */
export function sessionDigest(value: string): boolean {
  return /^[0-9a-f]{64}$/.test(value);
}

/** Mirror the configured profile owner grammar without allocating a second profile identity. */
function sessionProfile(value: string): boolean {
  return /^[A-Za-z0-9_-]{1,64}$/.test(value);
}

/** Reuse installation/profile owners and validate independent workspace/backend UUID namespaces. */
export function sessionScope(value: SessionScope): boolean {
  return (
    /^[0-9a-f]{32}$/.test(value.installation) &&
    sessionProfile(value.profile) &&
    [value.workspace, value.backend].every(sessionUuid)
  );
}

/** Reject Windows device names, traversal, drive/stream syntax and ambiguous components. */
function component(value: string): boolean {
  return (
    value.length > 0 &&
    value !== '.' &&
    value !== '..' &&
    !/[<>:"/\\|?*\u0000-\u001f\u007f]/.test(value) &&
    !/[. ]$/.test(value) &&
    !/^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(value) &&
    Number.isFinite(utf8Bytes(value))
  );
}

/** Accept backend-relative descriptors; approved roots and reparse containment remain server-owned. */
export function sessionFile(value: SessionFile): boolean {
  return (
    component(value.filename) &&
    (value.subfolder === '' || value.subfolder.split('/').every(component)) &&
    (value.managedArtifact === null ||
      (value.managedArtifact.length > 0 && component(value.managedArtifact)))
  );
}

/** Validate bounded JSON graph facts; Comfy remains the graph semantics owner. */
export function sessionGraph(encoded: string): boolean {
  if (utf8Bytes(encoded) > SESSION_DOCUMENT_BYTES) return false;
  let graph: unknown;
  try {
    graph = JSON.parse(encoded);
  } catch {
    return false;
  }
  if (!graph || typeof graph !== 'object' || Array.isArray(graph)) return false;
  const pending: { value: unknown; depth: number }[] = [{ value: graph, depth: 0 }];
  while (pending.length) {
    const item = pending.pop();
    if (!item || item.depth > 64) return false;
    const value = item.value;
    if (typeof value === 'number' && !Number.isFinite(value)) return false;
    if (typeof value === 'string' && !Number.isFinite(utf8Bytes(value))) return false;
    if (value && typeof value === 'object') {
      for (const [key, child] of Object.entries(value)) {
        if (!Number.isFinite(utf8Bytes(key))) return false;
        pending.push({ value: child, depth: item.depth + 1 });
      }
    }
  }
  return true;
}

/** Resolve only below the caller-supplied user-data root; this spelling carries no IO authority. */
export function sessionDirectory(profile: string): string | null {
  return sessionProfile(profile) ? `sessions/${profile}/` : null;
}
