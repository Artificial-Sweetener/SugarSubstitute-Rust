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
/** Canonical mounted paint issues target identity and finite hard-line leases independently of measurement. */
import type { ReorderRect, ReorderMarkerRect } from './reorder-geometry.js';
import type { ReorderLineLease } from './reorder-line-layout.js';

/** Genuine target nodes carry only Core-issued numeric identity and original hard-line paint leases. */
export interface MountedReorderTarget {
  readonly node: HTMLElement;
  readonly kind: ReorderRect['kind'];
  readonly index: number;
  readonly blank?: number;
  readonly leases: readonly ReorderLineLease[];
  readonly origins: readonly number[];
}

/** Complete publication and unpublished carrier preparation are the only whole-paint observation coverages. */
export type ReorderMountedCoverage =
  | { readonly kind: 'complete' }
  | {
      readonly kind: 'carrier-candidates';
      readonly viewport: ReorderMarkerRect;
    };

/** Root layout always covers every semantic target; rich fragments contain only actual measured native output. */
interface ReorderMountedGeometry {
  readonly layout: readonly ReorderRect[];
  readonly footprints: ReadonlyMap<HTMLElement, readonly ReorderRect[]>;
}

/** Only complete observations may publish a Scene; sparse preparation explicitly names every omitted whole semantic ID. */
export type ReorderCompleteMountedObservation = ReorderMountedGeometry & {
  readonly coverage: 'complete';
};
/** Unpublished candidate output can complete only its original finite native observation. */
export type ReorderMountedObservation =
  | ReorderCompleteMountedObservation
  | (ReorderMountedGeometry & {
      readonly coverage: 'carrier-candidates';
      readonly omitted: ReadonlySet<number>;
      /** Complete only missing rich groups synchronously under this invocation's original root samples and authority. */
      complete(): ReorderCompleteMountedObservation | null;
    });
