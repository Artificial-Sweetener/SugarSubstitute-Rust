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
/** Validate editable exact-source coordinates without scanning the complete source. */
import { requireValue } from '../required.js';
import type { TagQuery } from './contracts.js';
/** Reject out-of-source, fractional, surrogate-interior and CRLF-interior editable positions explicitly. */
export function assertCompletionBoundary(source: string, position: number): void {
  if (
    !Number.isSafeInteger(position) ||
    position < 0 ||
    position > source.length ||
    (position > 0 &&
      position < source.length &&
      /[\uD800-\uDBFF]/u.test(requireValue(source[position - 1], 'boundaries')) &&
      /[\uDC00-\uDFFF]/u.test(requireValue(source[position], 'boundaries'))) ||
    (source[position - 1] === '\r' && source[position] === '\n')
  )
    throw new RangeError('Invalid completion UTF-16 source boundary');
}
/** Validate source-command ordering at the public semantic boundary; freshness remains the native command owner's responsibility. */
export function assertTagQuery(source: string, query: TagQuery): void {
  const { from, to } = query.replacementRange;
  for (const position of [from, to, query.activeTagEnd]) assertCompletionBoundary(source, position);
  if (from > to || to > query.activeTagEnd) throw new RangeError('Invalid completion source range');
}
