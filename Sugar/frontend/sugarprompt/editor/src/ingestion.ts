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
/** Ordinary ingestion composes genuine canonical preparation and one encoded command with continuous native start states. */
import { requireValue } from '../../core/src/required.js';
import { Transaction, type EditorState } from '@codemirror/state';
import { decodedRangeToSource, type SourceRange } from '../../core/src/index.js';
import {
  pasteImportAnnotations,
  pasteImportUserEvent,
  type PasteImportHistoryIntent,
} from './paste-import-history.js';
import type { TagLexicon } from '../../core/src/index.js';
import { encodeSource, sourceToDocument } from '../../core/src/line-endings.js';
import { richRendering } from './presentation.js';
import {
  exactReplacement,
  lineEndings,
  readExactSource,
  type ExactSourceReplacement,
} from './exact-source.js';
import {
  prepareNativeStructure,
  semanticStructures,
  installSemanticStructure,
} from './structure.js';
import { nativeDocumentValueAt } from './document-values.js';
import { prepareDocumentValueEdit } from './document-value-edit-issuance.js';
import type { PreparedDocumentValueEdit } from './document-value-edits.js';
import { prepareDocumentValuePaste } from './document-value-ingestion.js';
/** Exact source boundaries may not bisect a CRLF spelling. */
function insideCrLf(text: string, position: number): boolean {
  return text[position - 1] === '\r' && text[position] === '\n';
}
/** Keep changed spans from splitting a shared astral scalar. */
function insideScalar(text: string, position: number): boolean {
  const before = text.charCodeAt(position - 1),
    after = text.charCodeAt(position);
  return before >= 0xd800 && before <= 0xdbff && after >= 0xdc00 && after <= 0xdfff;
}
/** Find one minimal changed source span without splitting exact CRLF or equal astral code points. */
export function minimalChangedSourceRange(
  before: string,
  after: string,
): {
  readonly from: number;
  readonly beforeTo: number;
  readonly afterTo: number;
} {
  let from = 0,
    beforeTo = before.length,
    afterTo = after.length;
  while (from < beforeTo && from < afterTo && before[from] === after[from]) from++;
  while (beforeTo > from && afterTo > from && before[beforeTo - 1] === after[afterTo - 1]) {
    beforeTo--;
    afterTo--;
  }
  if (
    insideCrLf(before, from) ||
    insideCrLf(after, from) ||
    insideScalar(before, from) ||
    insideScalar(after, from)
  )
    from--;
  if (
    insideCrLf(before, beforeTo) ||
    insideCrLf(after, afterTo) ||
    insideScalar(before, beforeTo) ||
    insideScalar(after, afterTo)
  ) {
    beforeTo++;
    afterTo++;
  }
  return { from, beforeTo, afterTo };
}
/** Prepare a minimal exact-source correction so unrelated native projections retain their source ranges. */
export function minimalSourceReplacement(
  state: EditorState,
  text: string,
): ExactSourceReplacement | undefined {
  const original = readExactSource(state);
  if (original === text) return undefined;
  const changed = minimalChangedSourceRange(original, text);
  return exactReplacement(
    state,
    changed.from,
    changed.beforeTo,
    text.slice(changed.from, changed.afterTo),
  );
}
/** An outer preparation chain carries actual transactions; each starts at the preceding actual state. */
export interface PreparedNativeIngestion {
  readonly startState: EditorState;
  readonly transactions: readonly Transaction[];
  readonly edit: PreparedDocumentValueEdit | null;
  /** Actual post-insertion exact-source UTF-16 boundaries, retained only by deliberate import preparation. */
  readonly acceptedRange?: SourceRange;
}
/** Prepare current canonical structure off input without reconstructing a native transaction or editable state. */
export function prepareNativeIngestionState(state: EditorState): readonly Transaction[] {
  return state.field(semanticStructures).native
    ? []
    : [
        state.update({
          effects: installSemanticStructure.of({
            purpose: 'native',
            prepared: prepareNativeStructure(state),
          }),
          annotations: Transaction.addToHistory.of(false),
        }),
      ];
}
/** Insert ordinary text at exact-source UTF-16 boundaries; only admitted decoded values receive Core normalization. */
export function enteredTextInsertion(
  state: EditorState,
  from: number,
  to: number,
  text: string,
  lexicon?: TagLexicon,
  history?: PasteImportHistoryIntent,
): PreparedNativeIngestion {
  const logical = state.doc.toString(),
    endings = state.field(lineEndings);
  const start = sourceToDocument(logical, endings, from),
    end = sourceToDocument(logical, endings, to);
  if (from > to) throw new RangeError('Reversed insertion range');
  const canonical = state.facet(richRendering) || history?.phase === 'replacement';
  const transactions = canonical ? [...prepareNativeIngestionState(state)] : [];
  const current = transactions.at(-1)?.state ?? state;
  const native = current.field(semanticStructures, false)?.native;
  if (canonical && native) {
    const context = {
      values: native.nativeValues,
      semantics: native.documentSemantics,
      enabled: true,
      composing: false,
    };
    const admitted = nativeDocumentValueAt(current, context, {
      from: start,
      to: end,
    });
    if (admitted.status === 'ready') {
      const target = admitted.target,
        range = target.decodedRange;
      const provenance = prepareDocumentValuePaste(target, range, text, lexicon);
      const normal = provenance.normalization;
      const changed = minimalChangedSourceRange(target.scope.mapping.text, normal.text);
      const insertedEnd = range.from + text.length;
      const mapped = requireValue(normal.boundaryPositions[insertedEnd], 'ingestion');
      const caret = insideCrLf(normal.text, mapped) ? mapped + 1 : mapped;
      const outcome = prepareDocumentValueEdit(current, {
        context,
        target,
        range: { from: changed.from, to: changed.beforeTo },
        text: normal.text.slice(changed.from, changed.afterTo),
        publication: {
          purpose: 'paste',
          normalization: provenance,
          ...(history ? { pasteHistory: history } : {}),
        },
        selection: {
          ranges: [{ kind: 'decoded', anchor: caret, head: caret }],
          mainIndex: 0,
        },
      });
      if (outcome.status === 'rejected') throw new Error('decoded-ingestion-command-refused');
      transactions.push(...outcome.edit.transactions);
      const padding = target.scope.container.valueRange.from;
      const mapping =
        outcome.edit.preparedValues.containersById[target.scope.container.id]?.mapping;
      const acceptedRange =
        history && mapping
          ? decodedRangeToSource(mapping, {
              from: padding + requireValue(normal.boundaryPositions[range.from], 'ingestion'),
              to: padding + mapped,
            })
          : null;
      if (history && !acceptedRange) throw new Error('paste-import-insertion-boundary');
      return Object.freeze({
        startState: state,
        transactions: Object.freeze(transactions),
        edit: outcome.edit,
        ...(acceptedRange ? { acceptedRange: Object.freeze(acceptedRange) } : {}),
      });
    }
  }
  const original = readExactSource(current),
    proposed = original.slice(0, from) + text + original.slice(to);
  const insertedEnd = from + text.length,
    caret = insideCrLf(proposed, insertedEnd) ? insertedEnd + 1 : insertedEnd;
  const encoded = encodeSource(proposed);
  const selection = {
    anchor: sourceToDocument(encoded.logical, encoded.endings, caret),
  };
  const source = minimalSourceReplacement(current, proposed);
  if (source) delete source.userEvent;
  const userEvent = history ? pasteImportUserEvent(history) : 'input.paste';
  transactions.push(
    current.update({
      ...source,
      selection,
      ...(userEvent ? { userEvent } : {}),
      ...(history
        ? {
            annotations: [
              ...pasteImportAnnotations(current, history),
              ...(original === proposed ? [Transaction.addToHistory.of(false)] : []),
            ],
          }
        : {}),
    }),
  );
  return Object.freeze({
    startState: state,
    transactions: Object.freeze(transactions),
    edit: null,
    ...(history ? { acceptedRange: Object.freeze({ from, to: insertedEnd }) } : {}),
  });
}
