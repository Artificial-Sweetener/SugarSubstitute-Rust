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
/** Typed parenthesis scope/reclassification policy consumes the shared scanner and authoritative parser. */
import { requireValue } from '../required.js';
import type { SourceRange } from '../offsets.js';
import { parsePrompt } from '../prompt/parser.js';
import type { SourceNormalization, TypedSourceEdit } from './contracts.js';
import { isExplicitEmphasis } from './parentheses.js';
import { parenthesisPairs } from './parenthesis-pairs.js';
import { isEscaped } from './region-context.js';
import { applyReplacements } from './replacements.js';
/** Match one authored delimiter by escape-aware balance; classification later handles quoted/angle syntax. */
function matchingParenthesis(text: string, position: number, forward: boolean): number | null {
  let depth = 0;
  for (let index = position; index >= 0 && index < text.length; index += forward ? 1 : -1) {
    if (isEscaped(text, index)) continue;
    const character = text[index];
    if (character === (forward ? '(' : ')')) depth++;
    else if (character === (forward ? ')' : '(')) {
      depth--;
      if (depth === 0) return index;
    }
  }
  return null;
}
/** Prepared tag membership needs the whole parsed segment containing the newly completed shell. */
function containingSegment(text: string, range: SourceRange): SourceRange {
  return (
    parsePrompt(text).segments.find(
      (segment) => segment.contentRange.from <= range.from && range.to <= segment.contentRange.to,
    )?.contentRange ?? range
  );
}
/** Normalize newly balanced syntax only; keep inner shells untouched while an authored ancestor remains open. */
export function typedParenthesisScope(text: string, edit: TypedSourceEdit): SourceRange | null {
  if (edit.to - edit.from > 1 && edit.replacementText.length > 1) {
    if (!/[()]/u.test(edit.replacementText)) return null;
    const completed = parenthesisPairs(text).filter(
      (pair) => pair.depth === 1 && pair.from < edit.to && pair.close >= edit.from,
    );
    if (!completed.length) return null;
    const first = containingSegment(text, {
        from: requireValue(completed[0], 'typed-parentheses').from,
        to: requireValue(completed[0], 'typed-parentheses').close + 1,
      }),
      last = containingSegment(text, {
        from: requireValue(completed.at(-1), 'typed-parentheses').from,
        to: requireValue(completed.at(-1), 'typed-parentheses').close + 1,
      });
    return { from: first.from, to: last.to };
  }
  if (edit.to - edit.from !== 1 || edit.replacementText.length !== 1) return null;
  if (edit.replacementText === '(') {
    const close = matchingParenthesis(text, edit.from, true);
    return close === null ? null : containingSegment(text, { from: edit.from, to: close + 1 });
  }
  if (edit.replacementText !== ')') return null;
  const open = matchingParenthesis(text, edit.from, false);
  if (open === null) return null;
  const scope = containingSegment(text, { from: open, to: edit.to }),
    pairs = parenthesisPairs(text.slice(scope.from, scope.to));
  const closed = pairs.find(
    (pair) => pair.from === open - scope.from && pair.close === edit.from - scope.from,
  );
  if (
    closed &&
    closed.depth >
      pairs.filter((pair) => pair.from < closed.from && pair.close > closed.close).length + 1
  )
    return null;
  return scope;
}
/** Find the smallest complete outer storage-escaped shell containing the typed numeric character. */
function enclosingEscapedGroup(text: string, position: number): SourceRange | null {
  const stack: number[] = [];
  let result: SourceRange | null = null;
  for (let index = 0; index < text.length - 1; index++) {
    if (text[index] !== '\\' || isEscaped(text, index)) continue;
    if (text[index + 1] === '(') {
      stack.push(index);
      index++;
    } else if (text[index + 1] === ')' && stack.length) {
      const from = requireValue(stack.pop(), 'typed-parentheses'),
        to = index + 2;
      if (from < position && position < to && (!result || to - from < result.to - result.from))
        result = { from, to };
      index++;
    }
  }
  return result;
}
/** Remove exactly the outer escapes when one numeric edit completes valid explicit emphasis. */
export function reclassifyTypedEmphasis(
  text: string,
  edit: TypedSourceEdit,
): SourceNormalization | null {
  if (edit.to <= edit.from || !/[\d:.]$/u.test(edit.replacementText)) return null;
  const scope = enclosingEscapedGroup(text, edit.to - 1);
  if (!scope) return null;
  const candidate = '(' + text.slice(scope.from + 2, scope.to - 2) + ')';
  if (!isExplicitEmphasis(candidate)) return null;
  return applyReplacements(
    text,
    [
      { from: scope.from, to: scope.from + 1, text: '' },
      { from: scope.to - 2, to: scope.to - 1, text: '' },
    ],
    [{ ...scope, kind: 'escaped_literal_to_emphasis', nestingDepth: 1 }],
  );
}
