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
/** Prepared region indexes consume canonical scopes and native mapping tables. */
import { requireValue } from '../../core/src/required.js';
import type { RegionSeparator } from '../../core/src/index.js';
import type { PreparedNativeDocumentValues } from './document-values.js';
/** Bounds use logical CM UTF-16; separator/name ranges remain decoded scope-local UTF-16. */
export interface RegionTarget {
  readonly scopeId: string;
  readonly ordinal: number;
  readonly from: number;
  readonly to: number;
  readonly nameFrom: number | null;
  readonly nameTo: number | null;
  readonly separator: RegionSeparator;
}
/** Map each actual separator once; local ordinals never imply cross-value peer membership. */
export function prepareRegionTargets(
  values: PreparedNativeDocumentValues,
): readonly RegionTarget[] {
  const targets: RegionTarget[] = [];
  for (const entry of values.entries) {
    const scope = entry.scope,
      map = entry.semanticMapping;
    if (!scope || !map) continue;
    /** Parser bounds must be safe canonical endpoints, including doubled-quote and CRLF gaps. */
    const position = (decoded: number): number => {
      const logical = map.decodedToLogical[decoded];
      if (logical == null) throw new Error('region-endpoint-unmapped');
      return logical;
    };
    scope.document.regions.separators.forEach((separator, ordinal) => {
      targets.push(
        Object.freeze({
          scopeId: scope.id,
          ordinal,
          separator,
          from: position(separator.tokenRange.from),
          to: position(separator.tokenRange.to),
          nameFrom: separator.nameRange ? position(separator.nameRange.from) : null,
          nameTo: separator.nameRange ? position(separator.nameRange.to) : null,
        }),
      );
    });
  }
  return Object.freeze(targets);
}
/** Inclusive logical token lookup consumes the sorted prepared index without source traversal. */
export function regionAtPosition(
  targets: readonly RegionTarget[],
  position: number,
): RegionTarget | null {
  let from = 0,
    to = targets.length;
  while (from < to) {
    const middle = (from + to) >>> 1;
    if (requireValue(targets[middle], 'region-index').to < position) from = middle + 1;
    else to = middle;
  }
  const target = targets[from];
  return target && target.from <= position && position <= target.to ? target : null;
}
