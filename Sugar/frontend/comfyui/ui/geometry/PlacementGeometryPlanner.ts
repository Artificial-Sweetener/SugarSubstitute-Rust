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
/** Forecast one renderer presentation from an immutable authored layout baseline. */

import { isRecord } from '../types/common.js';
import type { ImportEntry, ImportLayoutGroup, ImportPayload } from '../import/PlacementPayload.js';
import {
  attachAuthoredLayoutBaselines,
  AUTHORED_LAYOUT_KEY,
  readAuthoredLayoutBaseline,
  type AuthoredLayoutBaseline,
  type AuthoredLayoutEntry,
} from './AuthoredLayoutBaseline.js';
import { solveAuthoredLayout, type LayoutRect } from './AuthoredLayoutSolver.js';
import {
  solveAuthoredGroupPresentationRect,
  toComfyGroupStorageRect,
} from './AuthoredGroupGeometry.js';
import {
  authoredNodePresentationRect,
  estimateCollapsedPresentationWidth,
} from './ComfyNodeGeometry.js';
import type { RendererGeometryPolicy } from './RendererGeometryPolicy.js';

export type GeometryRect = LayoutRect;

/** Return a renderer presentation clone suitable for both preview and insertion. */
export function planPlacementGeometry(
  payload: ImportPayload,
  policy: RendererGeometryPolicy,
): ImportPayload {
  const withBaseline = attachAuthoredLayoutBaselines(clonePayload(payload));
  if (policy.renderer === 'litegraph') return withBaseline;
  const baseline = readPrimaryBaseline(withBaseline.layout?.groups ?? []);
  if (!baseline) return withBaseline;
  const entries = [...(withBaseline.nodes ?? []), ...(withBaseline.markers ?? [])];
  const byIdentity = new Map(entries.map((entry) => [readEntryIdentity(entry), entry]));
  const measurements = Object.entries(baseline.entries)
    .map(([identity, authored]) => {
      const item = byIdentity.get(identity);
      if (!item) return null;
      const authoredPresentation = authoredNodePresentationRect(
        {
          x: baseline.origin[0] + authored.x,
          y: baseline.origin[1] + authored.y,
          w: authored.w,
          h: authored.h,
        },
        authored.collapsed,
        authored.title,
      );
      return {
        identity,
        item,
        authored: authoredPresentation,
        measured: forecastPresentationSize(authored, policy),
      };
    })
    .filter((value): value is NonNullable<typeof value> => value !== null);
  const solved = solveAuthoredLayout(measurements);
  for (const item of solved) writeSolvedEntry(item.item, item.solved, baseline, policy);
  const groups = (withBaseline.layout?.groups ?? []).map((group) => {
    const groupBaseline = readAuthoredLayoutBaseline(group.sugarcubes?.[AUTHORED_LAYOUT_KEY]);
    if (!groupBaseline) return group;
    const groupRects = solved
      .filter(({ identity }) => groupBaseline.entries[identity])
      .map(({ solved: rect }) => rect);
    return writeSolvedGroup(group, groupBaseline, groupRects, policy);
  });
  return {
    ...withBaseline,
    ...(withBaseline.layout ? { layout: { ...withBaseline.layout, groups } } : {}),
  };
}

function clonePayload(payload: ImportPayload): ImportPayload {
  return {
    ...payload,
    nodes: (payload.nodes ?? []).map(cloneEntry),
    markers: (payload.markers ?? []).map(cloneEntry),
    ...(payload.layout
      ? {
          layout: {
            ...payload.layout,
            groups: (payload.layout.groups ?? []).map((group) => ({
              ...group,
              ...(isRecord(group.sugarcubes) ? { sugarcubes: { ...group.sugarcubes } } : {}),
            })),
          },
        }
      : {}),
  };
}

function cloneEntry(entry: ImportEntry): ImportEntry {
  return { ...entry, ...(entry.layout ? { layout: { ...entry.layout } } : {}) };
}

function readPrimaryBaseline(groups: ImportLayoutGroup[]): AuthoredLayoutBaseline | null {
  for (const group of groups) {
    const baseline = readAuthoredLayoutBaseline(group.sugarcubes?.[AUTHORED_LAYOUT_KEY]);
    if (baseline) return baseline;
  }
  return null;
}

function forecastPresentationSize(
  authored: AuthoredLayoutEntry,
  policy: RendererGeometryPolicy,
): { w: number; h: number } {
  if (authored.collapsed) {
    const authoredWidth = estimateCollapsedPresentationWidth(authored.title, authored.w);
    return {
      w: Math.max(authoredWidth, policy.minimumExpandedNodeWidth),
      h: 30,
    };
  }
  return {
    w: Math.max(authored.w, policy.minimumExpandedNodeWidth),
    h: authored.h,
  };
}

function writeSolvedEntry(
  entry: ImportEntry,
  solved: LayoutRect,
  baseline: AuthoredLayoutBaseline,
  policy: RendererGeometryPolicy,
): void {
  if (!entry.layout) return;
  const identity = readEntryIdentity(entry);
  const authored = baseline.entries[identity];
  if (!authored) return;
  const usesTitleOffset = policy.renderer === 'vue' || authored.collapsed;
  entry.layout.pos = [solved.x, usesTitleOffset ? solved.y + 30 : solved.y];
  if (!authored.collapsed) {
    entry.layout.size = [Math.max(authored.w, policy.minimumExpandedNodeWidth), authored.h];
  } else {
    entry.layout.size = [authored.w, authored.h];
  }
  entry.layout.presentation = { ...solved };
}

function writeSolvedGroup(
  group: ImportLayoutGroup,
  baseline: AuthoredLayoutBaseline,
  solvedRects: LayoutRect[],
  policy: RendererGeometryPolicy,
): ImportLayoutGroup {
  const presentation = solveAuthoredGroupPresentationRect(baseline, solvedRects);
  if (!presentation) return group;
  const absolute = toComfyGroupStorageRect(presentation, policy.renderer);
  const origin = baseline.origin;
  const sugarcubes = isRecord(group.sugarcubes)
    ? { ...group.sugarcubes, bounds: { ...absolute } }
    : group.sugarcubes;
  return {
    ...group,
    bounding: [absolute.x - origin[0], absolute.y - origin[1], absolute.w, absolute.h],
    ...(sugarcubes ? { sugarcubes } : {}),
  };
}

function readEntryIdentity(entry: ImportEntry): string {
  const value = entry.symbol ?? entry.alias;
  return typeof value === 'string' ? value.trim() : '';
}
