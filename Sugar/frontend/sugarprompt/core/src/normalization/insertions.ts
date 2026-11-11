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
/** Source insertion preparation separates caret affinities from exact original-coordinate provenance. */
import { requireValue } from '../required.js';
import type { SourceNormalization } from './contracts.js';
import { identityNormalization } from './replacements.js';
/** One source-safe insertion; its boundary may deliberately remain before inserted text. */
export interface SourceInsertion {
  readonly position: number;
  readonly text: string;
  readonly affinity: 'before' | 'after';
}
/** Transient prepared insertion result; inverse positions map resulting UTF-16 boundaries to original source. */
export interface InsertionPreparation {
  readonly normalization: SourceNormalization;
  readonly originalPositions: readonly number[];
}
/** Apply unique ordered insertions while retaining both caret policy and original source provenance. */
export function prepareInsertions(
  text: string,
  insertions: readonly SourceInsertion[],
): InsertionPreparation {
  if (!insertions.length) {
    const normalization = identityNormalization(text);
    return Object.freeze({
      normalization,
      originalPositions: normalization.boundaryPositions,
    });
  }
  const changes = new Map<number, SourceInsertion>();
  for (const insertion of insertions) {
    if (
      !Number.isInteger(insertion.position) ||
      insertion.position < 0 ||
      insertion.position > text.length ||
      changes.has(insertion.position)
    )
      throw new RangeError('Invalid source insertion');
    changes.set(insertion.position, insertion);
  }
  const parts: string[] = [],
    boundaries: number[] = [],
    original: number[] = [];
  let target = 0;
  for (let position = 0; position <= text.length; position++) {
    const insertion = changes.get(position);
    if (insertion?.affinity === 'before') boundaries[position] = target;
    original[target] = position;
    if (insertion) {
      parts.push(insertion.text);
      for (let offset = 1; offset <= insertion.text.length; offset++)
        original[target + offset] = position;
      target += insertion.text.length;
    }
    if (!insertion || insertion.affinity === 'after') boundaries[position] = target;
    if (position < text.length) {
      parts.push(requireValue(text[position], 'insertions'));
      target++;
    }
  }
  return Object.freeze({
    normalization: Object.freeze({
      text: parts.join(''),
      boundaryPositions: Object.freeze(boundaries),
      transitions: Object.freeze([]),
      generatedEmphases: Object.freeze([]),
    }),
    originalPositions: Object.freeze(original),
  });
}
