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
/** Ordered source transformations share one linear boundary mapping owner. */
import { requireValue } from '../required.js';
import type { SourceNormalization, SourceReplacement, ParenthesisTransition } from './contracts.js';
/** Preserve exact text and identity mappings when no semantic rewrite applies. */
export function identityNormalization(text: string): SourceNormalization {
  return Object.freeze({
    text,
    boundaryPositions: Object.freeze(Array.from({ length: text.length + 1 }, (_, index) => index)),
    transitions: Object.freeze([]),
    generatedEmphases: Object.freeze([]),
  });
}
/** Apply disjoint edits in order; local mappings can preserve semantic character identities. */
export function applyReplacements(
  text: string,
  edits: readonly SourceReplacement[],
  transitions: readonly ParenthesisTransition[] = [],
): SourceNormalization {
  if (!edits.length && !transitions.length) return identityNormalization(text);
  const parts: string[] = [],
    boundaries: number[] = [];
  let source = 0,
    target = 0;
  for (const edit of edits) {
    if (
      edit.from < source ||
      edit.to < edit.from ||
      edit.to > text.length ||
      !Number.isInteger(edit.from) ||
      !Number.isInteger(edit.to)
    )
      throw new RangeError('Invalid normalization replacements');
    parts.push(text.slice(source, edit.from));
    for (let index = source; index <= edit.from; index++)
      boundaries[index] = target + index - source;
    target += edit.from - source;
    parts.push(edit.text);
    const length = edit.to - edit.from;
    if (edit.boundaryPositions && edit.boundaryPositions.length !== length + 1)
      throw new Error('Incomplete local boundary mapping');
    let previous = 0;
    for (let offset = 0; offset <= length; offset++) {
      const position = edit.boundaryPositions
        ? edit.boundaryPositions[offset]
        : offset === length
          ? edit.text.length
          : Math.min(offset, edit.text.length);
      if (
        position === undefined ||
        !Number.isSafeInteger(position) ||
        position < previous ||
        position > edit.text.length
      )
        throw new Error('Invalid local boundary mapping');
      boundaries[edit.from + offset] = target + position;
      previous = position;
    }
    source = edit.to;
    target += edit.text.length;
  }
  parts.push(text.slice(source));
  for (let index = source; index <= text.length; index++)
    boundaries[index] = target + index - source;
  return Object.freeze({
    text: parts.join(''),
    boundaryPositions: Object.freeze(boundaries),
    transitions: Object.freeze(transitions.map((transition) => Object.freeze({ ...transition }))),
    generatedEmphases: Object.freeze([]),
  });
}
/** Compose canonicalization and numeric formatting without losing original source boundaries. */
export function composeNormalizations(
  first: SourceNormalization,
  second: SourceNormalization,
): SourceNormalization {
  if (second.transitions.length)
    throw new Error('Numeric normalization cannot add parenthesis transitions');
  return Object.freeze({
    text: second.text,
    boundaryPositions: Object.freeze(
      first.boundaryPositions.map((position) =>
        requireValue(second.boundaryPositions[position], 'replacements'),
      ),
    ),
    transitions: first.transitions,
    generatedEmphases: Object.freeze(
      first.generatedEmphases.map((shell) =>
        Object.freeze({
          ...shell,
          from: requireValue(second.boundaryPositions[shell.from], 'replacements'),
          to: requireValue(second.boundaryPositions[shell.to], 'replacements'),
        }),
      ),
    ),
  });
}
