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
/** Real canonical source/selection/authorship/history fixture shared by reorder transaction owners. */
import { EditorSelection, type EditorState, type Transaction } from '@codemirror/state';
import type { PromptDocumentKind } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import {
  initializeNativeStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  prepareNativeReorder,
  type NativeReorderCapture,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-command.ts';
import { structureState } from './document-structure-fixture.ts';

/** Initialize through the sole structure owner and preserve a real directional native selection. */
export function reorderState(
  source: string,
  anchor = 0,
  head = anchor,
  kind: PromptDocumentKind = 'prompt',
) {
  const fixture = structureState(source, kind, authorshipExtension);
  const state = initializeNativeStructure(
    fixture.state.update({ selection: EditorSelection.range(anchor, head) }).state,
  );
  return { ...fixture, state };
}
/** Capture actual current scope membership; absent admission is a fixture failure, never fake authority. */
export function captureReorder(
  state: EditorState,
  position = state.selection.main.head,
): NativeReorderCapture {
  const semantic = state.field(semanticStructures).native!;
  const capture = prepareNativeReorder(
    state,
    {
      values: semantic.nativeValues,
      semantics: semantic.documentSemantics,
      enabled: true,
      composing: false,
    },
    { from: position, to: position },
  );
  if (!capture) throw new Error('reorder-fixture-admission');
  return capture;
}
/** Supported command/history dispatch applies each genuine actual transaction to its native start state. */
export function transactionState(start: EditorState) {
  let current = start;
  return {
    get state() {
      return current;
    },
    dispatch(transaction: Transaction) {
      if (transaction.startState !== current) throw new Error('reorder-fixture-continuity');
      current = transaction.state;
    },
  };
}
