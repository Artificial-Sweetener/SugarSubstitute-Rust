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
/** LoRA name-slot completion uses exact-source UTF-16 and retains scheduler fields independently of the model choice. */
import type { SourceRange } from '../offsets.js';

/** One current name prefix and complete token replacement, including its original valid scheduler suffix. */
export interface LoraQuery {
  readonly queryText: string;
  /** Exact-source UTF-16 typed prefix, ending at the caret. */
  readonly nameRange: SourceRange;
  /** Exact-source UTF-16 complete editable name field, stopping before scheduling fields or the token boundary. */
  readonly nameFieldRange: SourceRange;
  readonly replacementRange: SourceRange;
  readonly typedWeightText: string | null;
  readonly preservedSuffix: string | null;
  readonly hasClosingBracket: boolean;
}

/** Immutable marker/field preparation; the native caller supplies current exact-source UTF-16 selection. */
export interface PreparedLoraQueries {
  /** Resolve a name slot without parsing or traversing unrelated source; invalid editable boundaries reject. */
  queryAt(position: number, hasSelection: boolean): LoraQuery | null;
}

/** One exact-source UTF-16 replacement is applied through the caller's native document transaction. */
export interface LoraReplacement {
  readonly range: SourceRange;
  readonly text: string;
}
