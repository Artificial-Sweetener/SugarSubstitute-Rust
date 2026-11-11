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
/** Non-ignorable document errors adapt core candidate ownership without reparsing source. */
import type { PreparedDocumentSemantics } from '../../core/src/index.js';
import type { SourceRange } from '../../core/src/offsets.js';

/** Unsupported candidate markers have explanatory intent and exact-source UTF-16 bounds. */
export interface UnsupportedSceneDiagnostic {
  readonly kind: 'unsupported-scene-marker';
  readonly severity: 'error';
  readonly range: SourceRange;
  readonly message: 'Scenes aren’t supported in wildcard values.';
}

/** Preserve prepared range identities; candidate markers remain errors while being edited and have no mutation command. */
export function unsupportedSceneDiagnostics(
  semantics: PreparedDocumentSemantics,
): readonly UnsupportedSceneDiagnostic[] {
  return Object.freeze(
    semantics.unsupportedSceneMarkerRanges.map((range) =>
      Object.freeze({
        kind: 'unsupported-scene-marker' as const,
        severity: 'error' as const,
        range,
        message: 'Scenes aren’t supported in wildcard values.' as const,
      }),
    ),
  );
}
