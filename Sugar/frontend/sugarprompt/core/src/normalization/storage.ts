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
/** Storage ingestion composes parenthesis classification and canonical numeric formatting once. */
import type { SourceNormalization } from './contracts.js';
import { canonicalizeParentheses, type ParenthesisOptions } from './parentheses.js';
import { composeNormalizations } from './replacements.js';
import { normalizeWeights } from './weights.js';

/** Prepare stored source without editing it; maps and transitions retain original exact-source UTF-16 positions. */
export function normalizeForStorage(
  text: string,
  options: ParenthesisOptions = {},
): SourceNormalization {
  const parentheses = canonicalizeParentheses(text, options);
  return composeNormalizations(parentheses, normalizeWeights(parentheses.text));
}
