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
/** Canonical CSV display groups preserve authored scalar boundaries and exact source independently of rendering. */
import type { SourceRange } from '../offsets.js';
import type { WildcardSpan } from '../prompt/contracts.js';
import { unicodeLowerCase } from '../prompt/lower.js';

/** One literal display replacement spans an exact-source UTF-16 group with the same lowercase identity. */
export interface WildcardColumnAlias {
  readonly range: SourceRange;
  readonly text: string;
}

/** Prepare only differing canonical glyphs; equal lowercase-width boundaries retain ordinary native caret stops. */
export function prepareWildcardColumnAliases(
  span: WildcardSpan,
  matchedColumn: string | null,
): readonly WildcardColumnAlias[] {
  if (span.form !== 'csv' || matchedColumn === null) return Object.freeze([]);
  const column = span.csvColumn;
  if (column === null || unicodeLowerCase(column) !== unicodeLowerCase(matchedColumn))
    throw new RangeError('Canonical wildcard column identity mismatch');
  if (column === matchedColumn) return Object.freeze([]);
  const authored = Array.from(column),
    canonical = Array.from(matchedColumn),
    aliases: WildcardColumnAlias[] = [];
  let sourceIndex = 0,
    displayIndex = 0,
    from = span.contentRange.from + 5 + span.identifier.length;
  while (sourceIndex < authored.length && displayIndex < canonical.length) {
    let sourceText = '',
      text = '',
      sourceWidth = 0,
      displayWidth = 0;
    do {
      if (sourceWidth <= displayWidth) {
        const character = authored[sourceIndex++];
        if (character === undefined) throw new Error('Canonical wildcard source boundary mismatch');
        sourceText += character;
        sourceWidth += unicodeLowerCase(character).length;
      }
      if (displayWidth < sourceWidth) {
        const character = canonical[displayIndex++];
        if (character === undefined)
          throw new Error('Canonical wildcard display boundary mismatch');
        text += character;
        displayWidth += unicodeLowerCase(character).length;
      }
    } while (sourceWidth !== displayWidth);
    const to = from + sourceText.length;
    if (sourceText !== text)
      aliases.push(Object.freeze({ range: Object.freeze({ from, to }), text }));
    from = to;
  }
  if (sourceIndex !== authored.length || displayIndex !== canonical.length)
    throw new Error('Canonical wildcard terminal boundary mismatch');
  return Object.freeze(aliases);
}
