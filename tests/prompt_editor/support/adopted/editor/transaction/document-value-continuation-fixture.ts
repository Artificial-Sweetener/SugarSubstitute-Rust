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
/** Actual native issuer/state fixtures exercise continuation without another parser or publication authority. */
import { prepareTagReplacement } from '../../../../../../frontend/sugarprompt/core/src/completion/text.ts';
import { StateEffect } from '@codemirror/state';
import { valueState, valueTarget, decodedEdit, acceptedEdit } from './document-value-fixture.ts';
import { completionSession } from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import {
  semanticStructures,
  installSemanticStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  authorshipExtension,
  installAuthorship,
} from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { preparedAuthorship } from '../../../../../../frontend/sugarprompt/editor/src/authorship-ranges.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import type {
  PreparedCompletionResult,
  PreparedCompletionSplice,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-model.ts';
import type { PreparedDocumentValueEdit } from '../../../../../../frontend/sugarprompt/editor/src/document-value-edits.ts';

/** Derive specimen splice observations solely from the genuine native command transaction. */
function splice(
  edit: PreparedDocumentValueEdit,
  from: number,
  to: number,
): PreparedCompletionSplice {
  const transaction = edit.transactions[0]!;
  return {
    edit,
    from,
    to,
    insert: transaction.newDoc.slice(
      transaction.changes.mapPos(from, -1),
      transaction.changes.mapPos(to, 1),
    ),
    selection: transaction.newSelection,
    effects: transaction.effects,
  };
}
/** Keep mixed endings and astral neighbors while using independently declared literal candidate output. */
export function continuationFixture(kind: 'prompt' | 'wildcard-csv' = 'prompt', noOp = false) {
  const source =
    kind === 'prompt'
      ? noOp
        ? '\u{1f642}\r\nhands\rtail'
        : '\u{1f642}\r\nhan\rtail'
      : 'Name,Value\r\nrow,"han"';
  let state = valueState(source, kind).state;
  const caret = kind === 'prompt' ? (noOp ? 8 : 6) : 19;
  state = state.update({
    effects: StateEffect.appendConfig.of([
      semanticStructures,
      authorshipExtension,
      completionSession,
    ]),
    selection: { anchor: caret },
  }).state;
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(state),
    }),
  }).state;
  const native = state.field(semanticStructures).native!;
  const context = {
    enabled: true,
    composing: false,
    values: native.nativeValues,
    semantics: native.documentSemantics,
  };
  const target = valueTarget(state, context, caret);
  const effect = installAuthorship.of(preparedAuthorship([], []));
  const edit = acceptedEdit(state, {
    ...decodedEdit(
      context,
      target,
      kind === 'prompt' ? 4 : 0,
      kind === 'prompt' ? (noOp ? 9 : 7) : 3,
      'hands',
    ),
    publication: { purpose: 'completion', authorship: effect },
  });
  const from = kind === 'prompt' ? 3 : 16,
    to = caret;
  const acceptance = {
    edit,
    splice: kind === 'prompt' ? splice(edit, from, to) : null,
    right: null,
  };
  const result: PreparedCompletionResult = {
    from,
    to,
    query: { scope: target.scope, kind: 'tag', from, contentFrom: from, to },
    options: [
      {
        kind: 'tag',
        label: 'hands',
        detail: '',
        freshness: state.field(editorFreshness),
        caret,
        ghost: 'ds',
        acceptance: { keyboard: acceptance, pointer: acceptance },
      },
    ],
  };
  return {
    state,
    context,
    target,
    edit,
    effect,
    result,
    scope: { context, target },
  };
}

/** Distinct genuine retained and suffix-consuming variants publish different complete outputs. */
export function rightVariantFixture() {
  let state = continuationFixture().state;
  state = state.update({
    changes: { from: 6, insert: 'ds' },
    selection: { anchor: 6 },
    userEvent: 'input.type',
  }).state;
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(state),
    }),
  }).state;
  const native = state.field(semanticStructures).native!;
  const context = {
    values: native.nativeValues,
    semantics: native.documentSemantics,
    enabled: true,
    composing: false,
  };
  const target = valueTarget(state, context, 6);
  const left = acceptedEdit(state, {
    ...decodedEdit(context, target, 4, 7, 'hands'),
    publication: {
      purpose: 'completion',
      authorship: installAuthorship.of(preparedAuthorship([], [])),
    },
  });
  const right = acceptedEdit(state, {
    ...decodedEdit(context, target, 4, 9, 'hands'),
    publication: {
      purpose: 'completion',
      authorship: installAuthorship.of(preparedAuthorship([], [])),
    },
  });
  const acceptance = {
    edit: right,
    splice: splice(left, 3, 6),
    right: {
      splice: splice(right, 3, 8),
      match: prepareTagReplacement(
        target.scope.mapping.text,
        target.scope.tagQueries.queryAt(7, false)!,
        'hands',
        false,
      ).right!.match,
    },
  };
  const result: PreparedCompletionResult = {
    from: 3,
    to: 8,
    query: { scope: target.scope, kind: 'tag', from: 3, contentFrom: 3, to: 8 },
    options: [
      {
        kind: 'tag',
        label: 'hands',
        detail: '',
        freshness: state.field(editorFreshness),
        caret: 6,
        ghost: 'ds',
        acceptance: { keyboard: acceptance, pointer: acceptance },
      },
    ],
  };
  return { state, result, left, right, scope: { context, target } };
}
