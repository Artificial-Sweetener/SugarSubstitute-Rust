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
/** Canonical numeric source replacements delegate grammar to the one prompt parser. */
import { parsePrompt } from '../prompt/parser.js';
import type { SourceRange } from '../offsets.js';
import type { SourceNormalization, SourceReplacement } from './contracts.js';
import { formatWeight } from './decimals.js';
import { applyReplacements } from './replacements.js';

/** Map numeric source characters before converting scalar boundaries to exact-source UTF-16. */
function weightReplacement(source: string, range: SourceRange, value: string): SourceReplacement {
  const text = formatWeight(value),
    before = source.slice(range.from, range.to),
    boundaries: number[] = [];
  let units = 0,
    scalar = 0;
  for (const character of before) {
    const position = Math.min(scalar, text.length);
    boundaries[units] = position;
    if (character.length === 2) boundaries[units + 1] = Math.min(position + 1, text.length);
    units += character.length;
    scalar++;
  }
  boundaries[units] = text.length;
  return { ...range, text, boundaryPositions: boundaries };
}

/** Format all recognized emphasis and both LoRA weight fields; invalid source remains literal. */
export function normalizeWeights(text: string): SourceNormalization {
  const document = parsePrompt(text);
  const edits: SourceReplacement[] = document.emphasisSpans.map((span) =>
    weightReplacement(text, span.weightRange, span.weight),
  );
  for (const span of document.loraSpans) {
    edits.push(weightReplacement(text, span.firstWeightRange, span.firstWeight));
    if (span.secondWeightRange && span.secondWeight !== null)
      edits.push(weightReplacement(text, span.secondWeightRange, span.secondWeight));
  }
  return applyReplacements(
    text,
    edits.sort((a, b) => a.from - b.from),
  );
}
