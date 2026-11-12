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
/** One native READ reduces actual complete/sparse footprints to whole semantic groups and animation eligibility. */
import { requireValue } from '../../core/src/required.js';
import { admitReorderTargetLayout } from './reorder-layout-admission.js';
import { nativeReorderFootprint } from './reorder-native-footprint.js';
import type { PassiveReorderPaint } from './reorder-widget.js';
import type { ReorderLayoutReadAuthority } from './reorder-slot-layout-read.js';
import type {
  ReorderMountedCoverage,
  ReorderMountedObservation,
} from './reorder-mounted-target.js';
import type { ReorderMarkerRect, ReorderRect } from './reorder-geometry.js';
/** Every authored root of one semantic chip is admitted together; these request-local buffers never become an alternate layout cache. */
export interface ReorderPassiveGeometryGroup {
  readonly rects: ReorderRect[];
  readonly roots: HTMLElement[];
}
/** The caller retains lifecycle/viewport authority; this owner delegates the exact whole read or original partial native admission. */
export interface ReorderPassiveGeometryRequest {
  readonly viewport: ReorderMarkerRect;
  readonly held: number | null;
  readonly whole: {
    readonly authority: ReorderLayoutReadAuthority;
    readonly readTargets: typeof admitReorderTargetLayout;
    readonly coverage: ReorderMountedCoverage;
  } | null;
  readonly current: () => boolean;
}
/** Sparse candidate output may prepare hidden copies only; complete output alone supplies a published Scene. */
interface ReorderPassiveGeometryData {
  readonly admitted: ReadonlyMap<number, ReorderPassiveGeometryGroup>;
  readonly geometry: readonly ReorderRect[];
  readonly eligible: readonly ReorderRect[];
  readonly layout: readonly ReorderRect[] | null;
}
/** Complete native geometry alone may leave the caller's READ as a published Scene. */
export type ReorderCompletePassiveGeometry = ReorderPassiveGeometryData & {
  readonly coverage: 'complete';
};
/** A required candidate continuation is finite to this READ and cannot return another sparse result. */
export type ReorderPassiveGeometry =
  | ReorderCompletePassiveGeometry
  | (ReorderPassiveGeometryData & {
      readonly coverage: 'carrier-candidates';
      complete(): ReorderCompletePassiveGeometry | null;
    });
/** Preserve complete-group/cap/viewport policy while sharing actual observed root fragments and rich output. */
export function readReorderPassiveGeometry(
  paint: PassiveReorderPaint,
  request: ReorderPassiveGeometryRequest,
): ReorderPassiveGeometry | null {
  const observation = request.whole
    ? paint.observe(request.whole.authority, request.whole.readTargets, request.whole.coverage)
    : null;
  if (request.whole && (!observation || !request.current())) return null;
  return admitReorderPassiveGeometry(paint, request, observation);
}
/** Both initial and completed observations share the same whole-ID, fragment-cap and viewport eligibility owner. */
function admitReorderPassiveGeometry(
  paint: PassiveReorderPaint,
  request: ReorderPassiveGeometryRequest,
  observation: ReorderMountedObservation | null,
): ReorderPassiveGeometry | null {
  const viewport = request.viewport;
  const targets = paint.targets(),
    byTarget = request.whole ? null : admitReorderTargetLayout(targets);
  if (!request.whole && (!request.current() || !byTarget)) return null;
  const roots = targets.filter((target) => target.kind === 'chip').map((target) => target.node);
  const admitted = new Map<number, ReorderPassiveGeometryGroup>();
  let fragmentCount = 0;
  for (const root of roots) {
    const id = Number(root.dataset.reorderChip);
    if (!Number.isSafeInteger(id) || id < 0 || (!request.whole && id === request.held)) continue;
    if (observation?.coverage === 'carrier-candidates' && observation.omitted.has(id)) continue;
    const rects = observation
      ? (observation.footprints.get(root) ?? null)
      : nativeReorderFootprint(
          root,
          requireValue(
            requireValue(byTarget, 'reorder-passive-geometry-read').get(root),
            'reorder-passive-geometry-read',
          ),
          id,
          viewport,
        );
    if (
      !rects ||
      !rects.length ||
      rects.length > 256 ||
      (!request.whole &&
        rects.some(
          (rect) =>
            rect.left < viewport.left ||
            rect.right > viewport.right ||
            rect.top < viewport.top ||
            rect.bottom > viewport.bottom,
        ))
    ) {
      if (request.whole) return null;
      continue;
    }
    fragmentCount += rects.length;
    if (fragmentCount > 4096) return null;
    const entry = admitted.get(id) ?? { rects: [], roots: [] };
    entry.roots.push(root);
    entry.rects.push(...rects);
    admitted.set(id, entry);
  }
  for (const root of roots) {
    const id = Number(root.dataset.reorderChip),
      entry = admitted.get(id);
    if (entry && !entry.roots.includes(root)) {
      if (request.whole) return null;
      admitted.delete(id);
    }
  }
  if (!admitted.size && observation?.coverage !== 'carrier-candidates') return null;
  const geometry = Object.freeze(Array.from(admitted.values()).flatMap((entry) => entry.rects));
  const eligible = request.whole
    ? new Set(
        Array.from(admitted.entries())
          .filter(
            ([id, entry]) =>
              id !== request.held &&
              entry.rects.every(
                (rect) =>
                  rect.left >= viewport.left &&
                  rect.right <= viewport.right &&
                  rect.top >= viewport.top &&
                  rect.bottom <= viewport.bottom,
              ),
          )
          .map(([id]) => id),
      )
    : null;
  const measured = {
    admitted,
    geometry,
    eligible: eligible ? geometry.filter((rect) => eligible.has(rect.index)) : geometry,
    layout: observation?.layout ?? null,
  };
  if (observation?.coverage !== 'carrier-candidates') return { ...measured, coverage: 'complete' };
  return {
    ...measured,
    coverage: 'carrier-candidates',
    complete: () => {
      if (!request.current()) return null;
      const full = observation.complete();
      if (!full || !request.current()) return null;
      const result = admitReorderPassiveGeometry(paint, request, full);
      if (result && result.coverage !== 'complete')
        throw new Error('reorder-candidate-completion-coverage');
      return result;
    },
  };
}
