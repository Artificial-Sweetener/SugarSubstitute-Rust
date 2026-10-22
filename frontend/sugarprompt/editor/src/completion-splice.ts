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
/** Off-input preparation retains the complete consumed prefix alongside normalized native output, including exact metadata. */
import { requireValue } from '../../core/src/required.js';
import type { EditorState } from '@codemirror/state';
import type { SourceRange, TagLexicon } from '../../core/src/index.js';
import { prepareDocumentValueEdit } from './document-value-edit-issuance.js';
import { nativeDocumentValueAt } from './document-values.js';
import { prepareDocumentValuePaste } from './document-value-ingestion.js';
import { minimalChangedSourceRange } from './ingestion.js';
import { semanticStructures } from './structure.js';
import type {
  PreparedCompletionEdit,
  PreparedCompletionScope,
  PreparedCompletionSplice,
} from './prompt-completion-model.js';
/** Logical-document UTF-16 bounds include the whole consumed query, even when the optimized edit shares a prefix with the candidate. */
export function prepareCompletionSplice(
  edit: import('./document-value-edits.js').PreparedDocumentValueEdit,
  from: number,
  to: number,
): PreparedCompletionSplice {
  const transaction = edit.transactions[0];
  if (!transaction) throw new Error('completion-splice-transaction-missing');
  transaction.changes.iterChangedRanges((changedFrom, changedTo) => {
    from = Math.min(from, changedFrom);
    to = Math.max(to, changedTo);
  });
  return Object.freeze({
    edit,
    from,
    to,
    insert: transaction.newDoc.slice(
      transaction.changes.mapPos(from, -1),
      transaction.changes.mapPos(to, 1),
    ),
    selection: transaction.newSelection,
    effects: transaction.effects,
  });
}
/** Resolve the actual current empty caret once; structured storage never gains a whole-source fallback. */
export function prepareCompletionScope(state: EditorState): PreparedCompletionScope | null {
  const semantic = state.field(semanticStructures).native;
  if (!semantic || state.readOnly || !state.selection.main.empty) return null;
  const context = Object.freeze({
      values: semantic.nativeValues,
      semantics: semantic.documentSemantics,
      enabled: true,
      composing: false,
    }),
    caret = state.selection.main.head,
    admitted = nativeDocumentValueAt(state, context, {
      from: caret,
      to: caret,
    });
  return admitted.status === 'ready' ? Object.freeze({ context, target: admitted.target }) : null;
}
/** Convert decoded scope-local UTF-16 only through the canonical semantic map; unsafe scalar and encoding endpoints refuse. */
export function completionPosition(scope: PreparedCompletionScope, decoded: number): number | null {
  return (
    requireValue(scope.target.entry.semanticMapping, 'completion-splice').decodedToLogical[
      decoded
    ] ?? null
  );
}
/** Encode an actual decoded completion through the single native producer; structured candidates retain no old-storage continuation splice. */
export function prepareScopedCompletionEdit(
  state: EditorState,
  scope: PreparedCompletionScope,
  range: SourceRange,
  text: string,
  consumed: SourceRange | null,
  normalization: {
    readonly lexicon: TagLexicon | undefined;
  } | null = null,
): PreparedCompletionEdit | null {
  const provenance = normalization
    ? prepareDocumentValuePaste(scope.target, range, text, normalization.lexicon)
    : null;
  const normal = provenance?.normalization;
  const changed = normal
    ? minimalChangedSourceRange(scope.target.scope.mapping.text, normal.text)
    : null;
  const caret = normal
    ? requireValue(normal.boundaryPositions[range.from + text.length], 'completion-splice')
    : range.from + text.length;
  const outcome = prepareDocumentValueEdit(state, {
    context: scope.context,
    target: scope.target,
    range: changed ? { from: changed.from, to: changed.beforeTo } : range,
    text: changed
      ? requireValue(normal, 'completion-splice').text.slice(changed.from, changed.afterTo)
      : text,
    selection: {
      ranges: [
        {
          kind: 'decoded',
          anchor: caret,
          head: caret,
        },
      ],
      mainIndex: 0,
    },
    publication: {
      purpose: 'completion',
      ...(provenance ? { normalization: provenance } : {}),
    },
  });
  if (outcome.status === 'rejected') return null;
  const from = consumed ? completionPosition(scope, consumed.from) : null,
    to = consumed ? completionPosition(scope, consumed.to) : null;
  return Object.freeze({
    edit: outcome.edit,
    splice:
      scope.context.values.scopes.values.kind === 'prompt' &&
      from !== null &&
      to !== null &&
      outcome.edit.transactions.length > 0
        ? prepareCompletionSplice(outcome.edit, from, to)
        : null,
    right: null,
  });
}
