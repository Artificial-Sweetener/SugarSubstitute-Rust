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
/** Actual native issuer/state fixtures exercise command receipts and accepted completion edits. */
import { StateEffect, type EditorState } from '@codemirror/state';
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
import type { PreparedCompletionResult } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-model.ts';

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
  /** Issue the literal candidate edit for whichever state accepts it. */
  const issue = (current: EditorState) =>
    acceptedEdit(current, {
      ...decodedEdit(
        context,
        target,
        kind === 'prompt' ? 4 : 0,
        kind === 'prompt' ? (noOp ? 9 : 7) : 3,
        'hands',
      ),
      publication: { purpose: 'completion', authorship: effect },
    });
  const edit = issue(state);
  const from = kind === 'prompt' ? 3 : 16,
    to = caret;
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
        prepareEdit: issue,
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
