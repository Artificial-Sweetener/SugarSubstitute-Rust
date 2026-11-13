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
/** Session-issued finite primary press identity; no source text or caret is retained. */
import { snapshotPresentation } from './native-snapshot-policy.js';
import {
  StateEffect,
  StateField,
  Transaction,
  type EditorState,
  type Extension,
} from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { editorFreshness } from './freshness.js';
/** Publish one session-issued primary press identity without retaining source or caret. */
export const setReorderGrabCursor = StateEffect.define<object | null>();
/** Current native source/context/selection retirement removes cursor intent in the same transaction. */
export const reorderGrabCursor = StateField.define<object | null>({
  create: () => null,
  update(current, transaction) {
    if (
      transaction.startState.field(editorFreshness) !== transaction.state.field(editorFreshness) ||
      transaction.selection
    )
      current = null;
    for (const effect of transaction.effects)
      if (effect.is(setReorderGrabCursor)) current = effect.value;
    return current;
  },
});
/** Current token owns explicit cursor retirement; a late old callback cannot clear a newer press. */
export function releaseReorderGrabCursor(state: EditorState, token: object): Transaction | null {
  return state.field(reorderGrabCursor, false) === token
    ? state.update({
        effects: setReorderGrabCursor.of(null),
        annotations: [Transaction.addToHistory.of(false), snapshotPresentation.of(true)],
      })
    : null;
}
/** Supported editor attributes and scoped theme override descendant LoRA/wildcard pointer cursors. */
export const reorderGrabCursorExtension: Extension = [
  reorderGrabCursor,
  EditorView.editorAttributes.compute(
    [reorderGrabCursor],
    (state): Record<string, string> =>
      state.field(reorderGrabCursor) ? { 'data-reorder-grab': 'true' } : {},
  ),
  EditorView.theme({
    '&[data-reorder-grab="true"], &[data-reorder-grab="true"] .cm-content, &[data-reorder-grab="true"] .cm-content *':
      { cursor: 'grabbing !important' },
  }),
];
