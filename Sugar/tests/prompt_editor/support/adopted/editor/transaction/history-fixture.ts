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
/** A real headless CodeMirror state proves history policy, effect inversion and exact-source coordinates without DOM ownership. */
import {
  EditorSelection,
  EditorState,
  Text,
  Transaction,
  type TransactionSpec,
} from '@codemirror/state';
import { undo, redo, undoDepth, redoDepth } from '@codemirror/commands';
import {
  encodeSource,
  sourceToDocument,
  documentToSource,
} from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import { codePointBoundaries } from '../../../../../../frontend/sugarprompt/core/src/offsets.ts';
import {
  exactSourceExtension,
  lineEndings,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  promptHistory,
  historyBoundary,
  historyTransactions,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts';

/** All fixture observations use exact-source UTF-16, with CodeMirror solely storing document, selection and history. */
export interface HistoryFrame {
  readonly source: string;
  readonly anchor: number;
  readonly head: number;
  readonly undoDepth: number;
  readonly redoDepth: number;
}

/** Drive the actual native state/command boundary; no test-owned source, caret or history implementation exists. */
export class HistoryFixture {
  state: EditorState;

  /** Install the same policy and native newline inversion as the mounted editor. */
  constructor(source: string) {
    const encoded = encodeSource(source);
    this.state = EditorState.create({
      doc: Text.of(encoded.logical.split('\n')),
      selection: EditorSelection.cursor(encoded.logical.length),
      extensions: [exactSourceExtension(source), promptHistory],
    });
  }

  /** Apply the supported transaction batch, including history-owned associated selection restoration. */
  dispatch = (transaction: Transaction): void => {
    this.state = historyTransactions([transaction]).at(-1)!.state;
  };

  /** Native transaction specifications remain the one source-edit owner. */
  update(specification: TransactionSpec): void {
    this.dispatch(this.state.update(specification));
  }

  /** Model committed native input with a controlled timestamp and logical LF; composition and DOM behavior belong to browser proof. */
  type(text: string, elapsed = 0): void {
    const selection = this.state.selection.main;
    this.update({
      changes: { from: selection.from, to: selection.to, insert: text },
      selection: EditorSelection.cursor(selection.from + text.length),
      userEvent: 'input.type',
      annotations: Transaction.time.of(1_000_000 + elapsed),
    });
  }

  /** The pure proof deletes one scalar or a selection; real grapheme/structural deletion remains native-browser-owned. */
  delete(direction: 'backward' | 'forward', elapsed = 0): void {
    const selection = this.state.selection.main;
    let from = selection.from,
      to = selection.to;
    if (selection.empty) {
      const positions = codePointBoundaries(this.state.doc.toString());
      const index = positions.indexOf(selection.head);
      if (index < 0) throw new Error('Fixture scalar boundary unavailable');
      if (direction === 'backward') from = positions[Math.max(0, index - 1)]!;
      else to = positions[Math.min(positions.length - 1, index + 1)]!;
    }
    if (from !== to)
      this.update({
        changes: { from, to },
        selection: EditorSelection.cursor(from),
        userEvent: `delete.${direction}`,
        annotations: Transaction.time.of(1_000_000 + elapsed),
      });
  }

  /** Exact-source UTF-16 selection uses the established logical-document conversion. */
  select(anchor: number, head: number): void {
    const logical = this.state.doc.toString(),
      endings = this.state.field(lineEndings);
    this.update({
      selection: EditorSelection.single(
        sourceToDocument(logical, endings, anchor),
        sourceToDocument(logical, endings, head),
      ),
      userEvent: 'select',
    });
  }

  /** Execute the real native history command and its actual associated-selection transaction adapter. */
  restore(action: 'undo' | 'redo'): boolean {
    return (action === 'undo' ? undo : redo)({
      state: this.state,
      dispatch: this.dispatch,
    });
  }

  /** Explicit native command intent can close a group even when source and selection do not change. */
  boundary(): void {
    const transaction = historyBoundary(this.state);
    if (transaction) this.dispatch(transaction);
  }

  /** Observe native exact source, directional selection and history availability without accessing private history fields. */
  snapshot(): HistoryFrame {
    const logical = this.state.doc.toString(),
      endings = this.state.field(lineEndings),
      selection = this.state.selection.main;
    return {
      source: readExactSource(this.state),
      anchor: documentToSource(logical, endings, selection.anchor),
      head: documentToSource(logical, endings, selection.head),
      undoDepth: undoDepth(this.state),
      redoDepth: redoDepth(this.state),
    };
  }
}
