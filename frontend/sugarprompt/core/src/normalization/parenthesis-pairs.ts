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
/** Balanced authoring shells outside quotes/angles, distinct from comma-segment bracket grammar. */
import { requireValue } from '../required.js';
import { isStructuralQuote } from '../prompt/text.js';
/** One unescaped balanced parenthesis shell; exact-source UTF-16 offsets and authored depth. */
export interface ParenthesisPair {
  readonly from: number;
  readonly close: number;
  readonly depth: number;
}
/** Scan every balanced shell while retaining depth contributed by incomplete outer parentheses. */
export function parenthesisPairs(source: string): readonly ParenthesisPair[] {
  const stack: number[] = [],
    pairs: ParenthesisPair[] = [];
  let quote: string | null = null,
    angles = 0;
  for (let index = 0; index < source.length; index++) {
    const character = requireValue(source[index], 'parenthesis-pairs');
    if (character === '\\') {
      index++;
      continue;
    }
    if (quote !== null) {
      if (character === quote && isStructuralQuote(source, index)) quote = null;
      continue;
    }
    if (character === '<') {
      angles++;
      continue;
    }
    if (character === '>' && angles) {
      angles--;
      continue;
    }
    if ((character === "'" || character === '"') && !angles && isStructuralQuote(source, index)) {
      quote = character;
      continue;
    }
    if (angles) continue;
    if (character === '(') stack.push(index);
    else if (character === ')' && stack.length)
      pairs.push({
        from: requireValue(stack.pop(), 'parenthesis-pairs'),
        close: index,
        depth: stack.length + 1,
      });
  }
  return pairs.sort((a, b) => a.from - b.from);
}
