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
/**
 * Own parsing, identity remapping, and coordinate transforms for placement payloads.
 */

import { readVector2 } from '../graph/VectorUtils.js';
import { isRecord } from '../types/common.js';
import type { PreviewLayout } from '../overlays/PlacementHelpers.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';
import type { GraphId } from '../types/graph.js';

export interface ImportEntryLayout extends PreviewLayout {
  id?: unknown;
  pos?: unknown;
  size?: unknown;
}

export interface ImportEntry extends UnknownRecord {
  id?: unknown;
  symbol?: unknown;
  alias?: unknown;
  class_type?: unknown;
  layout?: ImportEntryLayout | null;
  inputs?: UnknownRecord;
  widget_values?: UnknownRecord | null;
  extras?: UnknownRecord;
}

export interface ImportConnection extends UnknownRecord {
  from?: UnknownRecord;
  to?: UnknownRecord;
}

export interface ImportGroupMetadata extends UnknownRecord {
  managed?: unknown;
  cube_id?: unknown;
  instance_id?: unknown;
  markers?: UnknownRecord;
  nodes?: unknown;
  bounds?: UnknownRecord;
}

export interface ImportLayoutGroup extends UnknownRecord {
  title?: unknown;
  bounds?: unknown;
  sugarcubes?: ImportGroupMetadata | null;
}

export interface ImportLayout extends UnknownRecord {
  origin?: unknown;
  groups?: ImportLayoutGroup[];
  cube?: UnknownRecord;
}

export interface ImportPayload extends UnknownRecord {
  nodes?: ImportEntry[];
  markers?: ImportEntry[];
  connections?: ImportConnection[];
  subgraphs?: UnknownRecord[];
  layout?: ImportLayout | null;
  cube?: UnknownRecord;
  warnings?: unknown[];
  default_alias?: unknown;
}

export interface IdMaps {
  nodeIdMap?: Map<string, GraphId>;
  markerIdMap?: Map<string, GraphId>;
}

interface PreparePayloadOptions {
  shift?: Vec2;
  targetOrigin?: Vec2 | null;
  remapInstanceIds?: boolean;
}

/** Return only object records from an untrusted collection boundary. */
function readRecordArray(value: unknown): UnknownRecord[] {
  return Array.isArray(value) ? value.filter(isRecord) : [];
}

/** Normalize one untrusted node or marker entry without mutating its source. */
function readImportEntry(value: UnknownRecord): ImportEntry {
  const rawLayout = isRecord(value.layout) ? value.layout : null;
  const { title: rawTitle, ...layoutRest } = rawLayout ?? {};
  const layout: ImportEntryLayout | null = rawLayout
    ? {
        ...layoutRest,
        ...(typeof rawTitle === 'string' ? { title: rawTitle } : {}),
      }
    : null;
  return {
    ...value,
    ...(layout ? { layout } : {}),
    ...(isRecord(value.inputs) ? { inputs: value.inputs } : {}),
    ...(isRecord(value.widget_values) ? { widget_values: value.widget_values } : {}),
    ...(isRecord(value.extras) ? { extras: value.extras } : {}),
  };
}

/** Parse an untrusted prepared-import response into a typed payload. */
export function readImportPayload(value: unknown): ImportPayload | null {
  if (!isRecord(value)) return null;
  const rawLayout = isRecord(value.layout) ? value.layout : null;
  const layout: ImportLayout | null = rawLayout
    ? {
        ...rawLayout,
        groups: readRecordArray(rawLayout.groups).map((group) => ({
          ...group,
          ...(isRecord(group.sugarcubes) ? { sugarcubes: { ...group.sugarcubes } } : {}),
        })),
      }
    : null;
  return {
    ...value,
    nodes: readRecordArray(value.nodes).map(readImportEntry),
    markers: readRecordArray(value.markers).map(readImportEntry),
    connections: readRecordArray(value.connections),
    subgraphs: readRecordArray(value.subgraphs),
    ...(layout ? { layout } : {}),
    ...(isRecord(value.cube) ? { cube: value.cube } : {}),
  };
}

/** Summarize one prepared import for user-facing feedback. */
export function buildImportSummary(data: ImportPayload): string {
  const nodeCount = data.nodes?.length ?? 0;
  const markerCount = data.markers?.length ?? 0;
  const connectionCount = data.connections?.length ?? 0;
  return `Nodes: ${nodeCount}, markers: ${markerCount}, connections: ${connectionCount}`;
}

/** Collect every cube identity represented by a prepared import. */
export function collectCubeIdsFromPayload(payload: ImportPayload | null): Set<string> {
  const cubeIds = new Set<string>();
  if (!payload) return cubeIds;
  for (const group of payload.layout?.groups ?? []) {
    const cubeId =
      typeof group.sugarcubes?.cube_id === 'string' ? group.sugarcubes.cube_id.trim() : '';
    if (cubeId) cubeIds.add(cubeId);
  }
  const payloadCubeId =
    typeof payload.cube?.cube_id === 'string' ? payload.cube.cube_id.trim() : '';
  if (payloadCubeId) cubeIds.add(payloadCubeId);
  return cubeIds;
}

/** Create a graph-local instance identity for each placement operation. */
function createRuntimeInstanceId(): string {
  const cryptoRef =
    (typeof window !== 'undefined' ? window.crypto : null) ||
    (typeof globalThis !== 'undefined' ? globalThis.crypto : null);
  if (cryptoRef && typeof cryptoRef.randomUUID === 'function') {
    const generated = cryptoRef.randomUUID();
    if (typeof generated === 'string' && generated.trim()) return generated;
  }
  const time = Date.now().toString(36);
  const random = Math.random().toString(36).slice(2, 10);
  return `inst_${time}_${random}`;
}

/** Read normalized marker identifiers from managed group metadata. */
function readGroupMarkerIds(markers: unknown): string[] {
  if (!isRecord(markers)) return [];
  return [
    ...(Array.isArray(markers.inputs) ? markers.inputs : []),
    ...(Array.isArray(markers.outputs) ? markers.outputs : []),
  ].map((value) => String(value));
}

/** Assign fresh instance identities to placed groups and their markers. */
function remapPlacementInstanceIds(
  layout: ImportLayout | null,
  markers: ImportEntry[],
): { layout: ImportLayout | null; markers: ImportEntry[] } {
  if (!layout || !layout.groups || markers.length === 0) return { layout, markers };

  const markerInstanceIds = new Map<string, string>();
  const oldToNewInstanceIds = new Map<string, string>();
  const managedGroupInstanceIds: string[] = [];
  const groups = layout.groups.map((group) => {
    const sugarcubes = isRecord(group.sugarcubes) ? { ...group.sugarcubes } : null;
    if (!sugarcubes || sugarcubes.managed === false) return group;
    const oldInstanceId =
      typeof sugarcubes.instance_id === 'string' ? sugarcubes.instance_id.trim() : '';
    const nextInstanceId = createRuntimeInstanceId();
    sugarcubes.instance_id = nextInstanceId;
    if (oldInstanceId && !oldToNewInstanceIds.has(oldInstanceId)) {
      oldToNewInstanceIds.set(oldInstanceId, nextInstanceId);
    }
    managedGroupInstanceIds.push(nextInstanceId);
    for (const markerId of readGroupMarkerIds(sugarcubes.markers)) {
      markerInstanceIds.set(markerId, nextInstanceId);
    }
    return { ...group, sugarcubes };
  });

  const defaultInstanceId = managedGroupInstanceIds.length === 1 ? managedGroupInstanceIds[0] : '';
  const remappedMarkers = markers.map((entry) => {
    const markerId = entry.layout?.id ?? entry.id;
    const markerKey = markerId != null ? String(markerId) : '';
    const widgetValues = isRecord(entry.widget_values) ? { ...entry.widget_values } : null;
    let nextInstanceId = markerKey ? markerInstanceIds.get(markerKey) : undefined;
    if (!nextInstanceId && widgetValues) {
      const existingInstanceId =
        typeof widgetValues.instance_id === 'string' ? widgetValues.instance_id.trim() : '';
      nextInstanceId = oldToNewInstanceIds.get(existingInstanceId);
    }
    nextInstanceId ||= defaultInstanceId || undefined;
    if (!nextInstanceId) return entry;
    return {
      ...entry,
      widget_values: { ...(widgetValues ?? {}), instance_id: nextInstanceId },
    };
  });

  return { layout: { ...layout, groups }, markers: remappedMarkers };
}

/** Prepare a SugarCubes import payload for insertion into the live graph. */
export function prepareGraphInsertionPayload(
  payloadValue: unknown,
  { shift = [0, 0], targetOrigin = null, remapInstanceIds = true }: PreparePayloadOptions = {},
): ImportPayload | null {
  const payload = readImportPayload(payloadValue);
  if (!payload) return null;
  const [shiftX, shiftY] = shift;
  const existingOrigin = Array.isArray(payload.layout?.origin) ? payload.layout.origin : [0, 0];
  const nextOrigin = readVector2(
    Array.isArray(targetOrigin) ? targetOrigin : existingOrigin,
    existingOrigin[0],
    existingOrigin[1],
  );
  const shiftEntry = (entry: ImportEntry): ImportEntry => {
    const layout = entry.layout ? { ...entry.layout } : null;
    if (layout && Array.isArray(layout.pos)) {
      layout.pos = [Number(layout.pos[0]) + shiftX, Number(layout.pos[1]) + shiftY];
    }
    if (layout && Array.isArray(layout.size)) {
      layout.size = [Number(layout.size[0]), Number(layout.size[1])];
    }
    return { ...entry, layout };
  };
  const nodes = (payload.nodes ?? []).map(shiftEntry);
  let markers = (payload.markers ?? []).map(shiftEntry);
  let layout = payload.layout ? { ...payload.layout } : null;
  if (layout) {
    layout = { ...layout, origin: nextOrigin };
    if (Array.isArray(layout.groups)) {
      layout.groups = layout.groups.map((group) => {
        const shiftedGroup = { ...group };
        if (isRecord(group.sugarcubes)) {
          const sugarcubes = { ...group.sugarcubes };
          if (isRecord(sugarcubes.bounds)) {
            const bounds = { ...sugarcubes.bounds };
            const boundX = Number(bounds.x);
            const boundY = Number(bounds.y);
            if (Number.isFinite(boundX)) bounds.x = boundX + shiftX;
            if (Number.isFinite(boundY)) bounds.y = boundY + shiftY;
            sugarcubes.bounds = bounds;
          }
          shiftedGroup.sugarcubes = sugarcubes;
        }
        return shiftedGroup;
      });
    }
    if (remapInstanceIds) {
      const remapped = remapPlacementInstanceIds(layout, markers);
      layout = remapped.layout;
      markers = remapped.markers;
    }
  }
  return { ...payload, nodes, markers, layout };
}

/** Shift a placement preview payload into its committed graph location. */
export function buildShiftedPlacementPayload(
  payload: unknown,
  shift: Vec2,
  targetOrigin: Vec2,
): ImportPayload | null {
  return prepareGraphInsertionPayload(payload, {
    shift,
    targetOrigin,
    remapInstanceIds: true,
  });
}
