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
/** CodeMirror owns text; this field owns only original newline spellings and their history inversion. */
import { requireValue } from '../../core/src/required.js';
import {
  StateEffect,
  StateField,
  Text,
  Transaction,
  type EditorState,
  type Extension,
  type TransactionSpec,
} from '@codemirror/state';
import { invertedEffects, isolateHistory } from '@codemirror/commands';
import {
  decodeSource,
  encodeSource,
  sourceToDocument,
  type LineEnding,
} from '../../core/src/line-endings.js';
/** Replace newline spellings in the transaction's resulting document; no source text is retained. */
const restoreEndings = StateEffect.define<readonly LineEnding[]>();
/** Traverse original newline slots once; insertions add LF metadata without an argument-count limit. */
function changedEndings(
  endings: readonly LineEnding[],
  transaction: Transaction,
): readonly LineEnding[] {
  if (!transaction.docChanged) return endings;
  const result: LineEnding[] = [];
  let cursor = 0;
  transaction.changes.iterChanges((from, to, _newFrom, _newTo, inserted) => {
    const index = transaction.startState.doc.lineAt(from).number - 1;
    const removed = transaction.startState.doc.lineAt(to).number - 1 - index;
    for (; cursor < index; cursor++) result.push(requireValue(endings[cursor], 'exact-source'));
    for (let line = 1; line < inserted.lines; line++) result.push('\n');
    cursor = index + removed;
  });
  for (; cursor < endings.length; cursor++)
    result.push(requireValue(endings[cursor], 'exact-source'));
  return result;
}
/** One spelling per logical newline. History effects restore this metadata alongside CodeMirror text. */
export const lineEndings = StateField.define<readonly LineEnding[]>({
  /** Initialize LF for ordinary construction; exact loading provides a field initializer. */
  create(state) {
    return Array.from({ length: state.doc.lines - 1 }, (): LineEnding => '\n');
  },
  /** Follow document changes, then apply the final history/explicit-source metadata effect. */
  update(endings, transaction) {
    let next = changedEndings(endings, transaction);
    for (const effect of transaction.effects) if (effect.is(restoreEndings)) next = effect.value;
    if (next.length !== transaction.newDoc.lines - 1)
      throw new Error('Line-ending invariant violated');
    return next;
  },
});
/** Install exact loading metadata and native history inversion, without a custom history stack. */
export function exactSourceExtension(source: string): Extension {
  const encoded = encodeSource(source);
  return [
    lineEndings.init(() => encoded.endings),
    invertedEffects.of((transaction) => {
      const before = transaction.startState.field(lineEndings),
        after = transaction.state.field(lineEndings);
      let rewritesNewline = false;
      transaction.changes.iterChanges((from, to, _newFrom, _newTo, inserted) => {
        if (
          inserted.lines > 1 ||
          transaction.startState.doc.lineAt(from).number !==
            transaction.startState.doc.lineAt(to).number
        )
          rewritesNewline = true;
      });
      // Undo reconstructs logical newlines as LF even when forward spelling metadata compared equal.
      return !rewritesNewline &&
        before.length === after.length &&
        before.every((ending, index) => ending === after[index])
        ? []
        : [restoreEndings.of(before)];
    }),
  ];
}
/** Read exact source from authoritative document plus newline spelling metadata. */
export function readExactSource(state: EditorState): string {
  return decodeSource(state.doc.toString(), state.field(lineEndings));
}
/** Prepare an exact-source UTF-16 replacement, rejecting invalid ranges and CRLF-interior boundaries. */
export interface ExactSourceReplacement extends TransactionSpec {
  readonly effects: readonly StateEffect<readonly LineEnding[]>[];
}
/** Prepare one source-spelling replacement with an explicit standalone history boundary. */
export function exactReplacement(
  state: EditorState,
  from: number,
  to: number,
  source: string,
): ExactSourceReplacement {
  return prepareExactReplacementRange(state, from, to).replace(source);
}
/** Immutable derived replacement slot for one native state; callers must validate freshness before applying its edits. */
export interface PreparedExactReplacementRange {
  /** Encode only candidate text, sharing unchanged newline metadata across alternatives for the same slot. */
  replace(source: string): ExactSourceReplacement;
}
/** Resolve exact-source UTF-16 boundaries once when preparing multiple alternatives, outside input and paint. */
export function prepareExactReplacementRange(
  state: EditorState,
  from: number,
  to: number,
): PreparedExactReplacementRange {
  if (from > to) throw new RangeError('Reversed replacement range');
  const logical = state.doc.toString(),
    endings = state.field(lineEndings);
  const start = sourceToDocument(logical, endings, from),
    end = sourceToDocument(logical, endings, to);
  const first = state.doc.lineAt(start).number - 1;
  const last = state.doc.lineAt(end).number - 1;
  return {
    /** Candidate preparation retains only resolved positions and immutable newline metadata, not the native state. */
    replace(source) {
      const encoded = encodeSource(source);
      const unchanged =
        encoded.endings.length === last - first &&
        encoded.endings.every((ending, index) => ending === endings[first + index]);
      const next = unchanged
        ? endings
        : [...endings.slice(0, first), ...encoded.endings, ...endings.slice(last)];
      return {
        changes: {
          from: start,
          to: end,
          insert: Text.of(encoded.logical.split('\n')),
        },
        selection: { anchor: start + encoded.logical.length },
        effects: [restoreEndings.of(next)],
        annotations: isolateHistory.of('full'),
        userEvent: 'input.exact',
      };
    },
  };
}
/** Detect metadata mutations as well as text changes for read-only transaction filtering. */
export function changesExactSource(transaction: Transaction): boolean {
  return transaction.docChanged || transaction.effects.some((effect) => effect.is(restoreEndings));
}
