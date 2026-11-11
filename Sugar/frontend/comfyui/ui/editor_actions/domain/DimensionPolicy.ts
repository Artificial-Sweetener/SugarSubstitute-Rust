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
/** Own exact dimension inference and reference-compatible integer ratio arithmetic. */
export type DimensionSide = 'width' | 'height';
export interface DimensionPair {
  readonly stem: string;
  readonly width: string;
  readonly height: string;
}
export interface DimensionShape {
  readonly short_edge: number;
  readonly long_edge: number;
}
/** Retain Substitute's landscape ratio order. */
export const LANDSCAPE_RATIOS = [
  [1, 1],
  [5, 4],
  [4, 3],
  [3, 2],
  [16, 9],
  [2, 1],
  [21, 9],
] as const;
/** Retain Substitute's portrait ratio order. */
export const PORTRAIT_RATIOS = [
  [1, 1],
  [4, 5],
  [3, 4],
  [2, 3],
  [9, 16],
  [1, 2],
  [9, 21],
] as const;

/** Match exact width/height suffixes without joining unrelated source and target fields. */
export function inferDimensionPairs(names: readonly string[]): DimensionPair[] {
  const candidates = new Map<string, Partial<Record<DimensionSide, string>>>();
  for (const name of names) {
    const match = /^(?:(.+)_)?(width|height)$/.exec(name);
    if (!match) continue;
    const stem = match[1] ?? '';
    const side = match[2] === 'width' ? 'width' : 'height';
    const candidate = candidates.get(stem) ?? {};
    candidate[side] ??= name;
    candidates.set(stem, candidate);
  }
  return [...candidates].flatMap(([stem, pair]) =>
    pair.width && pair.height ? [{ stem, width: pair.width, height: pair.height }] : [],
  );
}

/** Match Python's ties-to-even rounding used by Substitute's dimension commands. */
export function roundDimension(value: number): number {
  const lower = Math.floor(value);
  return value - lower === 0.5 ? lower + Math.abs(lower % 2) : Math.round(value);
}

/** Calculate only the other edge; widget constraints remain with the native field owner. */
export function ratioDimension(
  anchor: number,
  side: DimensionSide,
  width: number,
  height: number,
): number | null {
  if (![anchor, width, height].every(Number.isFinite) || width <= 0 || height <= 0) return null;
  const result = roundDimension(
    side === 'width' ? (anchor * height) / width : (anchor * width) / height,
  );
  return Number.isSafeInteger(result) ? result : null;
}

/** Normalize positive integer dimensions into orientation-independent persisted edges. */
export function canonicalDimensions(width: number, height: number): DimensionShape | null {
  const edges = [roundDimension(width), roundDimension(height)];
  if (!edges.every((edge) => Number.isSafeInteger(edge) && edge > 0)) return null;
  return { short_edge: Math.min(...edges), long_edge: Math.max(...edges) };
}
