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
/** Source-backed scene headings retain semantic identity separately from visible title whitespace. */
import type { SourceRange } from '../offsets.js';
import type { SceneDocument, SceneMarker } from '../prompt/contracts.js';

/** Exact-source UTF-16 heading bounds conceal only the prefix and retain authored trailing whitespace. */
export interface SceneHeading {
  readonly marker: SceneMarker;
  readonly concealedRange: SourceRange;
  readonly visibleRange: SourceRange;
  readonly hostError: boolean;
  readonly error: boolean;
}

/** Derive title projection and error classification from the one parsed scene owner. */
export function prepareSceneHeadings(
  document: SceneDocument,
  errorKeys: ReadonlySet<string> = new Set(),
): readonly SceneHeading[] {
  return Object.freeze(
    document.scenes.map(({ marker }) => {
      const hostError = errorKeys.has(marker.normalizedKey);
      return Object.freeze({
        marker,
        concealedRange: Object.freeze({
          from: marker.lineRange.from,
          to: marker.titleRange.from,
        }),
        visibleRange: Object.freeze({
          from: marker.titleRange.from,
          to: Math.max(marker.titleRange.to, marker.lineRange.to),
        }),
        hostError,
        error: marker.duplicate || hostError,
      });
    }),
  );
}
