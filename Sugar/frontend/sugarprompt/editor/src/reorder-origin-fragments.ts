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
/** Core origin metadata prepares atomic output fragments without parsing or scanning emitted source. */
import { requireValue } from '../../core/src/required.js';
import type { ReorderOriginPiece, ReorderOutput, SourceRange } from '../../core/src/index.js';
const atomic = new WeakMap<ReorderOutput, readonly ReorderOriginPiece[]>();
/** Exact output identity retains one flattened canonical origin instruction sequence. */
export function atomicReorderOrigins(output: ReorderOutput): readonly ReorderOriginPiece[] {
  const previous = atomic.get(output);
  if (previous) return previous;
  const pieces: ReorderOriginPiece[] = [];
  for (const piece of output.origin.pieces) {
    if (piece.kind !== 'composed') {
      pieces.push(piece);
      continue;
    }
    let cursor = piece.outputRange.from;
    for (const part of piece.components) {
      const length =
        part.kind === 'derived' ? part.text.length : part.sourceRange.to - part.sourceRange.from;
      pieces.push(
        Object.freeze(
          part.kind === 'derived'
            ? {
                kind: 'derived',
                reason: part.reason,
                outputRange: Object.freeze({
                  from: cursor,
                  to: cursor + length,
                }),
              }
            : {
                kind: part.kind,
                sourceRange: part.sourceRange,
                outputRange: Object.freeze({
                  from: cursor,
                  to: cursor + length,
                }),
              },
        ),
      );
      cursor += length;
    }
    if (cursor !== piece.outputRange.to) throw new Error('reorder-origin-component-length');
  }
  const prepared = Object.freeze(pieces);
  atomic.set(output, prepared);
  return prepared;
}
/** A changed literal paint unit uses local decoded UTF-16 while retaining original source provenance. */
export function reorderOriginFragments(
  output: ReorderOutput,
  range: SourceRange,
): readonly ReorderOriginPiece[] {
  if (
    ![range.from, range.to].every(Number.isSafeInteger) ||
    range.from < 0 ||
    range.to < range.from ||
    range.to > output.text.length
  )
    throw new Error('reorder-origin-fragment-range');
  const pieces = atomicReorderOrigins(output),
    result: ReorderOriginPiece[] = [];
  let low = 0,
    high = pieces.length;
  while (low < high) {
    const mid = (low + high) >>> 1;
    if (requireValue(pieces[mid], 'reorder-origin-fragments').outputRange.to <= range.from)
      low = mid + 1;
    else high = mid;
  }
  for (
    let at = low;
    at < pieces.length &&
    requireValue(pieces[at], 'reorder-origin-fragments').outputRange.from < range.to;
    at++
  ) {
    const piece = requireValue(pieces[at], 'reorder-origin-fragments'),
      from = Math.max(range.from, piece.outputRange.from),
      to = Math.min(range.to, piece.outputRange.to),
      outputRange = Object.freeze({
        from: from - range.from,
        to: to - range.from,
      });
    if (piece.kind === 'composed') throw new Error('reorder-origin-fragment-not-atomic');
    result.push(
      Object.freeze(
        piece.kind === 'derived'
          ? { kind: 'derived', reason: piece.reason, outputRange }
          : {
              kind: piece.kind,
              sourceRange: Object.freeze({
                from: piece.sourceRange.from + from - piece.outputRange.from,
                to: piece.sourceRange.from + to - piece.outputRange.from,
              }),
              outputRange,
            },
      ),
    );
  }
  return Object.freeze(result);
}
