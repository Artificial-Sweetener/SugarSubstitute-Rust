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
/** Own the immutable renderer-neutral layout baseline carried by cube instances. */

import { readVector2 } from '../graph/VectorUtils.js';
import { isRecord } from '../types/common.js';
import type {
  ImportEntry,
  ImportGroupMetadata,
  ImportLayout,
  ImportLayoutGroup,
  ImportPayload,
} from '../import/PlacementPayload.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';

/** Name the managed-group field that stores an instance's authored layout baseline. */
export const AUTHORED_LAYOUT_KEY = 'authored_layout';
const BASELINE_SCHEMA = 1;

export interface AuthoredLayoutEntry {
  x: number;
  y: number;
  w: number;
  h: number;
  collapsed: boolean;
  title: string;
}

export interface AuthoredLayoutGroupRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface AuthoredLayoutBaseline {
  schema: number;
  origin: Vec2;
  entries: Record<string, AuthoredLayoutEntry>;
  group: AuthoredLayoutGroupRect | null;
}

/** Attach a canonical baseline to managed group metadata without changing cube data. */
export function attachAuthoredLayoutBaselines(payload: ImportPayload): ImportPayload {
  const layout = payload.layout;
  if (!layout) return payload;
  const entries = [...(payload.nodes ?? []), ...(payload.markers ?? [])];
  const entryIndex = indexEntries(entries);
  const groups = (layout.groups ?? []).map((group) =>
    attachGroupBaseline(group, layout, entryIndex),
  );
  return {
    ...payload,
    layout: {
      ...layout,
      groups,
    },
  };
}

/** Return the first managed authored baseline carried by a prepared Cube payload. */
export function readPayloadAuthoredLayoutBaseline(
  payload: ImportPayload,
): AuthoredLayoutBaseline | null {
  for (const group of payload.layout?.groups ?? []) {
    const baseline = readAuthoredLayoutBaseline(group.sugarcubes?.[AUTHORED_LAYOUT_KEY]);
    if (baseline) return baseline;
  }
  return null;
}

/** Shift persisted instance origin while keeping local authored geometry immutable. */
export function shiftAuthoredLayoutBaseline(metadata: ImportGroupMetadata, shift: Vec2): void {
  const baseline = readAuthoredLayoutBaseline(metadata[AUTHORED_LAYOUT_KEY]);
  if (!baseline) return;
  metadata[AUTHORED_LAYOUT_KEY] = {
    ...baseline,
    origin: [baseline.origin[0] + shift[0], baseline.origin[1] + shift[1]],
  };
}

/** Parse persisted layout baseline metadata at the dynamic host boundary. */
export function readAuthoredLayoutBaseline(value: unknown): AuthoredLayoutBaseline | null {
  if (!isRecord(value) || Number(value.schema) !== BASELINE_SCHEMA || !isRecord(value.entries)) {
    return null;
  }
  const origin = readVector2(value.origin, Number.NaN, Number.NaN);
  if (!origin.every(Number.isFinite)) return null;
  const entries: Record<string, AuthoredLayoutEntry> = {};
  for (const [identity, rawEntry] of Object.entries(value.entries)) {
    const parsed = readEntryRect(rawEntry);
    if (identity && parsed) entries[identity] = parsed;
  }
  if (!Object.keys(entries).length) return null;
  return {
    schema: BASELINE_SCHEMA,
    origin,
    entries,
    group: readGroupRect(value.group),
  };
}

/** Translate a baseline to a new instance anchor without changing its authored shape. */
export function translateAuthoredLayoutBaseline(
  baseline: AuthoredLayoutBaseline,
  shift: Vec2,
): AuthoredLayoutBaseline {
  return {
    ...baseline,
    origin: [baseline.origin[0] + shift[0], baseline.origin[1] + shift[1]],
  };
}

function attachGroupBaseline(
  group: ImportLayoutGroup,
  layout: ImportLayout,
  entryIndex: Map<string, { identity: string; entry: ImportEntry }>,
): ImportLayoutGroup {
  const metadata = isRecord(group.sugarcubes) ? { ...group.sugarcubes } : null;
  if (!metadata || metadata.managed === false) return group;
  if (readAuthoredLayoutBaseline(metadata[AUTHORED_LAYOUT_KEY])) {
    return { ...group, sugarcubes: metadata };
  }
  const memberKeys = readMemberKeys(metadata);
  const selected = memberKeys.size
    ? [...entryIndex.entries()].filter(([key]) => memberKeys.has(key))
    : [...entryIndex.entries()];
  const entries: Record<string, AuthoredLayoutEntry> = {};
  const origin = readVector2(layout.origin, 0, 0);
  for (const [, indexed] of selected) {
    const parsed = readImportEntryRect(indexed.entry, origin);
    if (parsed) entries[indexed.identity] = parsed;
  }
  if (!Object.keys(entries).length) return { ...group, sugarcubes: metadata };
  metadata[AUTHORED_LAYOUT_KEY] = {
    schema: BASELINE_SCHEMA,
    origin,
    entries,
    group: readGroupRectVector(group.bounding),
  } satisfies AuthoredLayoutBaseline;
  return { ...group, sugarcubes: metadata };
}

function indexEntries(
  entries: ImportEntry[],
): Map<string, { identity: string; entry: ImportEntry }> {
  const indexed = new Map<string, { identity: string; entry: ImportEntry }>();
  for (const entry of entries) {
    const id = entry.layout?.id ?? entry.id;
    const identity = readEntryIdentity(entry);
    if (id != null && identity) indexed.set(String(id), { identity, entry });
    if (identity) indexed.set(identity, { identity, entry });
  }
  return indexed;
}

function readEntryIdentity(entry: ImportEntry): string {
  const value = entry.symbol ?? entry.alias;
  return typeof value === 'string' ? value.trim() : '';
}

function readMemberKeys(metadata: UnknownRecord): Set<string> {
  const keys = new Set<string>();
  if (Array.isArray(metadata.nodes)) {
    for (const value of metadata.nodes) keys.add(String(value));
  }
  if (isRecord(metadata.markers)) {
    for (const markerList of [metadata.markers.inputs, metadata.markers.outputs]) {
      if (Array.isArray(markerList)) {
        for (const value of markerList) keys.add(String(value));
      }
    }
  }
  return keys;
}

function readImportEntryRect(entry: ImportEntry, origin: Vec2): AuthoredLayoutEntry | null {
  const layout = entry.layout;
  if (!layout) return null;
  const pos = readVector2(layout.pos, Number.NaN, Number.NaN);
  const size = readVector2(layout.size, Number.NaN, Number.NaN);
  if (![...pos, ...size].every(Number.isFinite)) return null;
  const flags = isRecord(layout.flags)
    ? layout.flags
    : isRecord(layout.extra) && isRecord(layout.extra.flags)
      ? layout.extra.flags
      : null;
  return {
    x: pos[0] - origin[0],
    y: pos[1] - origin[1],
    w: size[0],
    h: size[1],
    collapsed: flags?.collapsed === true,
    title: typeof layout.title === 'string' ? layout.title : '',
  };
}

function readEntryRect(value: unknown): AuthoredLayoutEntry | null {
  if (!isRecord(value)) return null;
  const numbers = [value.x, value.y, value.w, value.h].map(Number);
  if (!numbers.every(Number.isFinite)) return null;
  return {
    x: numbers[0] ?? 0,
    y: numbers[1] ?? 0,
    w: numbers[2] ?? 0,
    h: numbers[3] ?? 0,
    collapsed: value.collapsed === true,
    title: typeof value.title === 'string' ? value.title : '',
  };
}

function readGroupRect(value: unknown): AuthoredLayoutGroupRect | null {
  if (!isRecord(value)) return null;
  const numbers = [value.x, value.y, value.w, value.h].map(Number);
  if (!numbers.every(Number.isFinite)) return null;
  return {
    x: numbers[0] ?? 0,
    y: numbers[1] ?? 0,
    w: numbers[2] ?? 0,
    h: numbers[3] ?? 0,
  };
}

function readGroupRectVector(value: unknown): AuthoredLayoutGroupRect | null {
  if (!Array.isArray(value) || value.length < 4) return null;
  const numbers = value.slice(0, 4).map(Number);
  if (!numbers.every(Number.isFinite)) return null;
  return {
    x: numbers[0] ?? 0,
    y: numbers[1] ?? 0,
    w: numbers[2] ?? 0,
    h: numbers[3] ?? 0,
  };
}
