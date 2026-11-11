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
/** Scene band grouping delegates all heading recognition to the canonical Core SceneDocument. */
import { requireValue } from '../../core/src/required.js';
import type { SceneDocument, SourceRange } from '../../core/src/index.js';
import { strip } from '../../core/src/prompt/text.js';
const empty: readonly SourceRange[] = Object.freeze([]);
/** Odd groups retain exact source UTF-16 operands; universal content consumes the first parity only when nonblank. */
export function prepareSceneBands(document: SceneDocument | null): readonly SourceRange[] {
  if (!document?.scenes.length) return empty;
  const first = strip(document.universalText) ? 1 : 0;
  return Object.freeze(
    document.scenes.flatMap((scene, index) =>
      (first + index) % 2
        ? [
            Object.freeze({
              from: scene.marker.titleRange.from,
              to: scene.contentRange.to,
            }),
          ]
        : [],
    ),
  );
}
/** Native wrapped lines share a hard-line owner; internal empty rows belong to a half-open Scene range, and terminal EOF adds none. */
export function sceneBandIntersectsLine(
  bands: readonly SourceRange[],
  from: number,
  to: number,
): boolean {
  let low = 0,
    high = bands.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (requireValue(bands[middle], 'scene-line-bands').to <= from) low = middle + 1;
    else high = middle;
  }
  const band = bands[low];
  return (
    !!band && (from === to ? band.from <= from && from < band.to : from < band.to && band.from < to)
  );
}
