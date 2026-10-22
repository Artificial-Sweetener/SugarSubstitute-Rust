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
/** Python-compatible physical line ownership shared by prompt and candidate semantics. */
import { requireValue } from '../required.js';
/** Exact-source UTF-16 bounds include the original physical line ending. */
export interface PhysicalLine {
  readonly from: number;
  readonly to: number;
  readonly text: string;
  /** Exact-source UTF-16 end of content, excluding the parser-owned physical ending. */
  readonly contentTo: number;
  /** Original Python-compatible physical separator, or the empty EOF spelling. */
  readonly ending: string;
}
/** Iterate Python-compatible records with exact endings; include-eof also represents empty source and trailing destinations. Default callers retain splitlines behavior. */
export function* physicalLines(
  source: string,
  emptyRecordPolicy: 'omit-eof' | 'include-eof' = 'omit-eof',
): Generator<PhysicalLine> {
  const lines =
    // eslint-disable-next-line no-control-regex -- Preserve exact Python-compatible control-code and whitespace boundaries.
    /[^\n\r\v\f\x1c-\x1e\x85\u2028\u2029]*(\r\n|[\n\r\v\f\x1c-\x1e\x85\u2028\u2029]|$)/gu;
  let contentAtEof = false;
  for (const match of source.matchAll(lines))
    if (match[0]) {
      const ending = requireValue(match[1], 'physical-lines');
      contentAtEof = ending === '';
      yield Object.freeze({
        from: match.index,
        to: match.index + match[0].length,
        text: match[0],
        contentTo: match.index + match[0].length - ending.length,
        ending,
      });
    }
  if (emptyRecordPolicy === 'include-eof' && !contentAtEof)
    yield Object.freeze({
      from: source.length,
      to: source.length,
      contentTo: source.length,
      text: '',
      ending: '',
    });
}
