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
/** Source transformation contracts; all ranges and boundary mappings use exact-source UTF-16. */
import type { SourceRange } from '../offsets.js';

/** A non-overlapping source replacement prepared by one semantic owner. */
export interface SourceReplacement extends SourceRange {
  readonly text: string;
  readonly boundaryPositions?: readonly number[];
}

/** One authored semantic transition, always expressed in the transformation's original source. */
export interface ParenthesisTransition extends SourceRange {
  readonly kind: 'implicit_emphasis' | 'known_literal_tag' | 'escaped_literal_to_emphasis';
  readonly nestingDepth: number;
}

/** Inserted source range after a typed edit, in exact-source UTF-16; replacement preserves its actual spelling. */
export interface TypedSourceEdit extends SourceRange {
  readonly replacementText: string;
}

/** Immutable transformed text plus mappings for every original exact-source UTF-16 boundary. */
export interface SourceNormalization {
  readonly text: string;
  readonly boundaryPositions: readonly number[];
  readonly transitions: readonly ParenthesisTransition[];
  /** Generated shells in resulting exact-source UTF-16, each retaining its own authored multiplier depth. */
  readonly generatedEmphases: readonly GeneratedEmphasis[];
}

/** Previously generated shell identity allows direct nesting to retain its original multiplier depth. */
export interface GeneratedEmphasis extends SourceRange {
  readonly nestingDepth: number;
}

/** Immutable exact-tag membership, prepared outside editor input and never backed by I/O. */
export interface TagLexicon {
  /** Answer exact prepared membership with the dataset's normalized tag identity rules. */
  contains(text: string): boolean;
}
