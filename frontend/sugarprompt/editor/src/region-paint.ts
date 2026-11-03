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
import { Facet } from '@codemirror/state';
import {
  REGION_PALETTE_BASE,
  formatRegionHex,
  parseRegionHex,
  type PreparedDocumentPromptScope,
} from '../../core/src/index.js';
import { prepareRegionColor, type PreparedRegionColor } from './region-color.js';

/** The pinned palette base, used when the host supplies no accent. */
export const DEFAULT_REGION_PALETTE = formatRegionHex(REGION_PALETTE_BASE);
/** The lowercase `#rrggbb` base of the regional palette: the host accent, or the pinned base. */
export const regionPalette = Facet.define<string, string>({
  /** Duplicate palette authority is an observable configuration invariant. */
  combine(values) {
    if (values.length > 1) throw new Error('Multiple region palette owners');
    return values[0] ?? DEFAULT_REGION_PALETTE;
  },
});

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
const scopes = new WeakMap<
  PreparedDocumentPromptScope,
  ReadonlyMap<string, readonly PreparedRegionPaint[]>
>();
/** Both native projection and reorder presentation borrow the exact same prepared colors; no pointer frame computes a palette. */
export function preparedRegionPaint(
  scope: PreparedDocumentPromptScope,
  palette: string = DEFAULT_REGION_PALETTE,
): readonly PreparedRegionPaint[] {
  const cached = scopes.get(scope);
  const previous = cached?.get(palette);
  if (previous) return previous;
  const base = parseRegionHex(palette);
  if (!base) throw new TypeError('Invalid region palette');
  const total = scope.document.regions.separators.length;
  const paints = Object.freeze(
    scope.document.regions.separators.map((_, ordinal) =>
      Object.freeze({
        scopeId: scope.id,
        ordinal,
        ...prepareRegionColor(ordinal, total, base),
      }),
    ),
  );
  scopes.set(scope, new Map([...(cached ?? []), [palette, paints]]));
  return paints;
}
