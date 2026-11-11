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
/** Source-free authored normalization observations are attached to their accepted native transactions. */
import { Annotation } from '@codemirror/state';
import type { SourceNormalization } from '../../core/src/index.js';

/** Typed implicit nesting is a presentation intent; history and bulk ingestion do not reproduce it. */
export const authoredImplicitDepth = Annotation.define<number>();

/** Prepare the maximum newly authored implicit nesting depth without exposing source text. */
export function authoredNesting(normalization: SourceNormalization): readonly Annotation<number>[] {
  const depth = normalization.transitions.reduce(
    (maximum, transition) =>
      transition.kind === 'implicit_emphasis'
        ? Math.max(maximum, transition.nestingDepth)
        : maximum,
    0,
  );
  return depth < 2 ? [] : [authoredImplicitDepth.of(depth)];
}
