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
/** Exact local-range validation and source-slice lifting own UTF-16 boundary correspondence. */
import type { SourceRange } from '../offsets.js';
import type { SourceNormalization } from './contracts.js';
import { applyReplacements, identityNormalization } from './replacements.js';

/** Recognize numeric UTF-16 endpoints that preserve physical CRLF units; scalar checks belong to the offset owner. */
export function isSourceBoundary(text: string, position: number): boolean {
  return (
    Number.isInteger(position) &&
    position >= 0 &&
    position <= text.length &&
    !(position > 0 && text[position - 1] === '\r' && text[position] === '\n')
  );
}

/** Validate a post-edit source range before preparing any semantic result. */
export function validateSourceRange(text: string, range: SourceRange): void {
  if (
    !Number.isInteger(range.from) ||
    !Number.isInteger(range.to) ||
    range.from < 0 ||
    range.to < range.from ||
    range.to > text.length
  )
    throw new RangeError('Invalid normalization range');
  for (const position of [range.from, range.to])
    if (!isSourceBoundary(text, position))
      throw new RangeError('Normalization boundary splits CRLF');
}

/** Normalize one source slice and shift its semantic ranges, leaving outside bytes exact. */
export function normalizeRange(
  text: string,
  range: SourceRange,
  normalize: (source: string) => SourceNormalization,
): SourceNormalization {
  validateSourceRange(text, range);
  if (range.from === range.to) return identityNormalization(text);
  const result = normalize(text.slice(range.from, range.to));
  const normalized = applyReplacements(
    text,
    [
      {
        ...range,
        text: result.text,
        boundaryPositions: result.boundaryPositions,
      },
    ],
    result.transitions.map((transition) => ({
      ...transition,
      from: range.from + transition.from,
      to: range.from + transition.to,
    })),
  );
  return Object.freeze({
    ...normalized,
    generatedEmphases: Object.freeze(
      result.generatedEmphases.map((shell) =>
        Object.freeze({
          ...shell,
          from: range.from + shell.from,
          to: range.from + shell.to,
        }),
      ),
    ),
  });
}
