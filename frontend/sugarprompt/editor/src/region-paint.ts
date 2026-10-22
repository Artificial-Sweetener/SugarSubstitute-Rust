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
/** One prepared canonical scope owns its regional palette for native rows, bounded marks and complete readonly row paint. */
import type { PreparedDocumentPromptScope } from '../../core/src/index.js';
import { prepareRegionColor, type PreparedRegionColor } from './region-color.js';

/** Native pseudo paint and canonical layer paint share one fixed local row-axis/width contract. */
export const regionRailMetrics = Object.freeze({
  axis: -3,
  width: 2,
  hoverWidth: 3.5,
});

/** A non-global canonical partition retains its source-bound scope identity and one prepared ordinal palette. */
export interface PreparedRegionPaint extends PreparedRegionColor {
  readonly scopeId: string;
  readonly ordinal: number;
}
const scopes = new WeakMap<PreparedDocumentPromptScope, readonly PreparedRegionPaint[]>();
/** Both native projection and reorder presentation borrow the exact same prepared colors; no pointer frame computes a palette. */
export function preparedRegionPaint(
  scope: PreparedDocumentPromptScope,
): readonly PreparedRegionPaint[] {
  const previous = scopes.get(scope);
  if (previous) return previous;
  const total = scope.document.regions.separators.length;
  const paints = Object.freeze(
    scope.document.regions.separators.map((_, ordinal) =>
      Object.freeze({
        scopeId: scope.id,
        ordinal,
        ...prepareRegionColor(ordinal, total),
      }),
    ),
  );
  scopes.set(scope, paints);
  return paints;
}
