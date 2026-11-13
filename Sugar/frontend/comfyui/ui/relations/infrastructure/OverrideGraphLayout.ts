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
/** Persist graph-card placement independently from executable nodes and shared field values. */
import { isRecord, type UnknownRecord } from '../../types/common.js';
import type { ComfyNode } from '../../types/graph.js';

const key = 'sugarcubes_override_card';
export interface OverrideGraphLayout {
  x: number;
  y: number;
  width: number;
}

/** Accept only finite graph coordinates and a usable card width from workflow metadata. */
export function readOverrideGraphLayout(extra: unknown): OverrideGraphLayout | null {
  const value = isRecord(extra) ? extra[key] : null;
  return isRecord(value) &&
    finite(value.x) &&
    finite(value.y) &&
    finite(value.width) &&
    value.width >= 240
    ? { x: value.x, y: value.y, width: value.width }
    : null;
}

/** Capture the native node's current drag and resize state without copying field values. */
export function captureOverrideGraphLayout(node: ComfyNode): OverrideGraphLayout {
  const x = node.pos?.[0],
    y = node.pos?.[1],
    width = node.size?.[0];
  return {
    x: finite(x) ? x : 32,
    y: finite(y) ? y : 96,
    width: finite(width) ? Math.max(240, width) : 400,
  };
}

/** Merge placement into host metadata while retaining other extensions' data. */
export function writeOverrideGraphLayout(
  extra: unknown,
  layout: OverrideGraphLayout,
): UnknownRecord {
  return { ...(isRecord(extra) ? extra : {}), [key]: layout };
}

/** Reject nonnumeric and nonfinite coordinates at the persistence boundary. */
function finite(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}
