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
/** Current semantic insertion occurrences adapt Core topology to one native hidden-layout lane snapshot. */
import { requireValue } from '../../core/src/required.js';
import {
  type ReorderPlacementTopology,
  sameReorderTarget,
  type ReorderOccurrence,
} from './reorder-placement-topology.js';
import {
  valueReorderVacancy,
  valueReorderAdjacentTarget,
  type ReorderState,
  type ReorderDropTarget,
} from '../../core/src/index.js';
import type { ReorderArrow } from './reorder-measurement-contracts.js';
/** Resolve current hidden occurrence, reading-order horizontal moves, adjacent physical lanes and boundary clamps. */
export function reorderKeyboardTarget(
  order: ReorderState,
  hidden: ReorderState,
  held: number,
  topology: ReorderPlacementTopology | null,
  key: ReorderArrow,
  preferredX: number | null,
  seed?: {
    readonly x: number;
    readonly y: number;
  },
): {
  readonly target: ReorderDropTarget;
  readonly preferredX: number;
} | null {
  const slot = valueReorderVacancy(order, hidden, held),
    all = topology?.slots ?? [];
  if (!slot) return null;
  const matching = all.filter((o) => sameReorderTarget(o.target, slot));
  const origin = [...matching].sort((a, b) => {
    const distance = (o: ReorderOccurrence): number =>
      preferredX !== null
        ? Math.abs(o.x - preferredX)
        : seed
          ? (o.x - seed.x) ** 2 + (o.y - seed.y) ** 2
          : 0;
    return distance(a) - distance(b);
  })[0];
  if (!origin) return null;
  let destination: ReorderOccurrence | undefined;
  const direction = key === 'ArrowLeft' || key === 'ArrowUp' ? -1 : 1;
  if (key === 'ArrowLeft' || key === 'ArrowRight') {
    let at = all.indexOf(origin) + direction;
    while (
      at >= 0 &&
      at < all.length &&
      sameReorderTarget(requireValue(all[at], 'reorder-keyboard-geometry').target, origin.target)
    )
      at += direction;
    destination = all[at];
    if (destination) {
      const target = valueReorderAdjacentTarget(
        hidden,
        origin.target,
        destination.target,
        direction,
      );
      destination = target
        ? direction > 0
          ? all.slice(at).find((o) => sameReorderTarget(o.target, target))
          : all.slice(0, at + 1).findLast((o) => sameReorderTarget(o.target, target))
        : undefined;
    }
  } else {
    const lane = origin.lane + direction,
      candidates = all.filter((o) => o.lane === lane),
      x = preferredX ?? origin.x;
    if (candidates.length) {
      const candidate = requireValue(
          [...candidates].sort((a, b) => Math.abs(a.x - x) - Math.abs(b.x - x))[0],
          'reorder-keyboard-geometry',
        ),
        target = valueReorderAdjacentTarget(hidden, origin.target, candidate.target, direction);
      destination = target
        ? candidates.find((occurrence) => sameReorderTarget(occurrence.target, target))
        : undefined;
    } else
      destination =
        direction < 0
          ? all.find((o) => o.lane === origin.lane)
          : all.filter((o) => o.lane === origin.lane).at(-1);
  }
  return destination &&
    !(
      sameReorderTarget(destination.target, origin.target) &&
      destination.x === (preferredX ?? origin.x)
    )
    ? { target: destination.target, preferredX: destination.x }
    : null;
}
