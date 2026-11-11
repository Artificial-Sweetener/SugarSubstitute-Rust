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
/** Source-position indexes prepare LoRA name slots without folding source offsets or scanning unrelated text during queries. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import { strip } from '../prompt/text.js';
import { parseWeight } from '../prompt/weights.js';
import { loraWeightFields } from '../prompt/lora-fields.js';
import { assertCompletionBoundary } from './boundaries.js';
import type { LoraQuery, PreparedLoraQueries } from './lora-contracts.js';
/** Query provenance retains immutable source only for the lifetime of a returned query. */
const querySources = new WeakMap<LoraQuery, string>();
/** Prepared replacements reject copied descriptors and stale source without reparsing during acceptance. */
export function assertPreparedLoraQuery(source: string, query: LoraQuery): void {
  if (querySources.get(query) !== source) throw new TypeError('Stale or unprepared LoRA query');
}
/** Each literal marker retains a static first separator and prepared scheduler suffix. */
interface NameSlot {
  readonly from: number;
  readonly closer: number;
  readonly nextGreater: number;
  readonly separator: number;
  readonly typedWeightText: string | null;
  readonly preservedSuffix: string | null;
}
/** Find the first position at/after a source boundary using one sorted prepared index. */
function lowerBound(positions: readonly number[], position: number): number {
  let lower = 0,
    upper = positions.length;
  while (lower < upper) {
    const middle = (lower + upper) >>> 1;
    if (requireValue(positions[middle], 'lora-query') < position) lower = middle + 1;
    else upper = middle;
  }
  return lower;
}
/** Prepare literal ASCII case-insensitive markers, structural characters and conservative open-token boundaries once. */
export function prepareLoraQueries(source: string): PreparedLoraQueries {
  if (typeof source !== 'string') throw new TypeError('Invalid LoRA completion source');
  const markers: number[] = [],
    less: number[] = [],
    greater: number[] = [],
    colons: number[] = [],
    endings: number[] = [];
  for (let position = 0; position < source.length; position++) {
    const character = requireValue(source[position], 'lora-query');
    if (character === '<') {
      less.push(position);
      if (/^<lora:$/iu.test(source.slice(position, position + 6))) markers.push(position);
    } else if (character === '>') greater.push(position);
    else if (character === ':') colons.push(position);
    if (',\r\n\t '.includes(character)) endings.push(position);
  }
  /** Return an absent sentinel without confusing a valid source zero with missing data. */
  const next = (positions: readonly number[], position: number): number =>
    positions[lowerBound(positions, position)] ?? -1;
  const slots: readonly NameSlot[] = markers.map((from) => {
    const nameFrom = from + 6,
      nextGreater = next(greater, nameFrom),
      nextLess = next(less, from + 1);
    const closer = nextGreater >= 0 && (nextLess < 0 || nextGreater < nextLess) ? nextGreater : -1;
    const separator = next(colons, nameFrom);
    let typedWeightText: string | null = null,
      preservedSuffix: string | null = null;
    if (separator >= 0 && (closer < 0 || separator < closer)) {
      const openEnd = next(endings, separator),
        bodyEnd = closer >= 0 ? closer : openEnd < 0 ? source.length : openEnd;
      const second = next(colons, separator + 1);
      const firstEnd = second >= 0 && second < bodyEnd ? second : bodyEnd;
      const weight = strip(source.slice(separator + 1, firstEnd));
      if (weight && parseWeight(weight, true) !== null) typedWeightText = weight;
      if (loraWeightFields(source, separator, bodyEnd))
        preservedSuffix = source.slice(separator, bodyEnd);
    }
    return Object.freeze({
      from,
      closer,
      nextGreater,
      separator,
      typedWeightText,
      preservedSuffix,
    });
  });
  return Object.freeze({
    /** Name-only caret lookup preserves exact source offsets even when Unicode case folding expands preceding characters. */
    queryAt(position: number, hasSelection: boolean): LoraQuery | null {
      assertCompletionBoundary(source, position);
      if (hasSelection) return null;
      // The six-character marker must fit through the caret's following character before it supersedes an earlier slot.
      const slot = slots[lowerBound(markers, position - 4) - 1];
      if (
        !slot ||
        position < slot.from + 6 ||
        (slot.nextGreater >= 0 && position > slot.nextGreater)
      )
        return null;
      const openEnd = next(endings, position);
      const bodyEnd = slot.closer >= 0 ? slot.closer : openEnd < 0 ? source.length : openEnd;
      const ownsWeight = slot.separator >= 0 && slot.separator < bodyEnd;
      if (ownsWeight && position > slot.separator) return null;
      const nameRange: SourceRange = Object.freeze({
        from: slot.from + 6,
        to: position,
      });
      const query: LoraQuery = Object.freeze({
        queryText: source.slice(nameRange.from, nameRange.to),
        nameRange,
        nameFieldRange: Object.freeze({
          from: nameRange.from,
          to: ownsWeight ? slot.separator : bodyEnd,
        }),
        replacementRange: Object.freeze({
          from: slot.from,
          to: slot.closer >= 0 ? slot.closer + 1 : bodyEnd,
        }),
        typedWeightText: ownsWeight ? slot.typedWeightText : null,
        preservedSuffix: ownsWeight ? slot.preservedSuffix : null,
        hasClosingBracket: slot.closer >= 0,
      });
      querySources.set(query, source);
      return query;
    },
  });
}
