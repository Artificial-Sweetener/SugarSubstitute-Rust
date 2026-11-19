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
  // Read code units in place: iterating code points allocates a string for each one, and a
  // session's graph text is several megabytes counted more than once for every capture.
  let bytes = 0;
  for (let index = 0; index < value.length; index += 1) {
    const unit = value.charCodeAt(index);
    if (unit < 0x80) bytes += 1;
    else if (unit < 0x800) bytes += 2;
    else if (unit < 0xd800 || unit > 0xdfff) bytes += 3;
    else {
      // A high surrogate and the low one after it are one four-byte code point.
      const low = unit <= 0xdbff ? value.charCodeAt(index + 1) : Number.NaN;
      if (!(low >= 0xdc00 && low <= 0xdfff)) return Infinity;
      bytes += 4;
      index += 1;
    }
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
  return boundedGraphValue(graph, 0);
}

/**
 * Accept finite numbers and well-formed text no deeper than 64 levels below the graph.
 *
 * It walks the parsed value in place: a graph holds hundreds of thousands of values, and a
 * record allocated for each of them made every capture pay for its own garbage collection.
 */
function boundedGraphValue(value: unknown, depth: number): boolean {
  if (depth > 64) return false;
  if (typeof value === 'number') return Number.isFinite(value);
  if (typeof value === 'string') return Number.isFinite(utf8Bytes(value));
  if (value === null || typeof value !== 'object') return true;
  if (Array.isArray(value)) {
    for (const child of value as readonly unknown[])
      if (!boundedGraphValue(child, depth + 1)) return false;
    return true;
  }
  const record = value as Record<string, unknown>;
  for (const key of Object.keys(record))
    if (!Number.isFinite(utf8Bytes(key)) || !boundedGraphValue(record[key], depth + 1))
      return false;
  return true;
}

/** Resolve only below the caller-supplied user-data root; this spelling carries no IO authority. */
export function sessionDirectory(profile: string): string | null {
  return sessionProfile(profile) ? `sessions/${profile}/` : null;
}
