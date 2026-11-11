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
/** Own the expand, contract and feather amount edited for a selection or a whole mask. */
import type { CanvasBrushOptions } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';

export type CanvasCoverageOperation = 'expand' | 'contract' | 'feather';

export interface CanvasCoverageModification {
  readonly operation: CanvasCoverageOperation;
  readonly pixels: number;
}

/** Operations in the order the operation selector lists them. */
export const CANVAS_COVERAGE_OPERATIONS: readonly CanvasCoverageOperation[] = Object.freeze([
  'expand',
  'contract',
  'feather',
]);

/** Every modification editor opens on Expand by four pixels. */
export const DEFAULT_COVERAGE_MODIFICATION: CanvasCoverageModification = Object.freeze({
  operation: 'expand',
  pixels: 4,
});

/** Smallest modification amount in whole pixels. */
export const COVERAGE_PIXELS_MIN = 1;
/** Largest modification amount in whole pixels. */
export const COVERAGE_PIXELS_MAX = 999;

/** Read an edited amount as a whole pixel count within 1..999; unreadable text keeps the last. */
export function coveragePixels(text: string, previous: number): number {
  const value = Number.parseInt(text, 10);
  if (!Number.isFinite(value)) return previous;
  return Math.min(COVERAGE_PIXELS_MAX, Math.max(COVERAGE_PIXELS_MIN, value));
}

/** Validate a coverage operation chosen in a control. */
export function coverageOperation(value: string): CanvasCoverageOperation | null {
  return CANVAS_COVERAGE_OPERATIONS.find((operation) => operation === value) ?? null;
}

/** Brush limits: size in document pixels, hardness and opacity in percent. */
export const BRUSH_LIMITS: Readonly<
  Record<keyof CanvasBrushOptions, { readonly min: number; readonly max: number }>
> = Object.freeze({
  size: { min: 1, max: 1000 },
  hardness: { min: 0, max: 100 },
  opacity: { min: 0, max: 100 },
});

/** Replace one brush option with a whole number inside its limits. */
export function withBrushOption(
  options: CanvasBrushOptions,
  field: keyof CanvasBrushOptions,
  value: number,
): CanvasBrushOptions {
  const { min, max } = BRUSH_LIMITS[field];
  const bounded = Number.isFinite(value) ? Math.min(max, Math.max(min, Math.round(value))) : min;
  return { ...options, [field]: bounded };
}
