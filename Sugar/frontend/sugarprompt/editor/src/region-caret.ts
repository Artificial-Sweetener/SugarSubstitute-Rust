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
/** Regional separator rows are structure, never an editing position: carets skip them and typed text lands in region content. */
import {
  EditorState,
  type Extension,
  type Transaction,
  type TransactionSpec,
} from '@codemirror/state';
import type { SourceRange } from '../../core/src/offsets.js';
import { richProjection } from './rich-projection.js';
import { RegionWidget } from './region-widget.js';

/**
 * Return the logical range of the painted separator row whose marker touches this position.
 * The painted projection is mapped through every edit, so rows stay known while the next
 * semantic preparation is still pending; raw rendering paints no rows and edits markers freely.
 */
function regionRowAt(state: EditorState, position: number): SourceRange | null {
  let row: SourceRange | null = null;
  state.field(richProjection).decorations.between(position, position, (from, to, decoration) => {
    if (decoration.spec.widget instanceof RegionWidget) row = { from, to };
  });
  return row;
}

/**
 * Resolve a caret that landed on a separator row the way the desktop prompt editor does: its edge
 * carets live on the neighbouring lines, so moving forward (and every pointer click) reaches the
 * start of that region's content and moving backward reaches the end of the previous content.
 * A final row without content keeps the caret at the marker's end, where typed text opens the
 * region's first content line.
 */
export function regionContentCaret(state: EditorState, position: number, forward: boolean): number {
  const row = regionRowAt(state, position);
  if (!row) return position;
  const line = state.doc.lineAt(row.from);
  const following = line.to < state.doc.length ? line.to + 1 : null;
  const previous = line.from > 0 ? line.from - 1 : null;
  return (forward ? following : (previous ?? following)) ?? row.to;
}

/** Typing at the end of a final separator row opens the region's first content line instead of extending the marker. */
function openRegionContent(transaction: Transaction): TransactionSpec | readonly TransactionSpec[] {
  if (
    !transaction.docChanged ||
    !transaction.isUserEvent('input.type') ||
    transaction.isUserEvent('input.type.compose')
  )
    return transaction;
  const start = transaction.startState;
  let insertion: number | null = null,
    changes = 0;
  transaction.changes.iterChanges((from, to, _fromB, _toB, inserted) => {
    changes++;
    if (from === to && inserted.length > 0 && inserted.sliceString(0, 1) !== '\n') insertion = from;
  });
  if (changes !== 1 || insertion === null || insertion !== start.doc.length) return transaction;
  if (regionRowAt(start, insertion)?.to !== insertion) return transaction;
  // The marker keeps its own line; the typed text follows it as the region's content.
  return [transaction, { changes: { from: insertion, insert: '\n' }, sequential: true }];
}

/** Native extension guarding typed input at separator rows; pointer and arrow owners use regionContentCaret. */
export const regionCaret: Extension = EditorState.transactionFilter.of(openRegionContent);
