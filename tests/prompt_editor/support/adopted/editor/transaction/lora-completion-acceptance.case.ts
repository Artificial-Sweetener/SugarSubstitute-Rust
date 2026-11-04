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
/** Native state qualifies whole-token LoRA acceptance, stale rows after edits and scheduler-boundary retirement. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, Text } from '@codemirror/state';
import { history, undo } from '@codemirror/commands';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import {
  semanticStructures,
  installSemanticStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { prepareCompletionScope } from '../../../../../../frontend/sugarprompt/editor/src/completion-splice.ts';
import type { PromptDocumentKind } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { createLoraCatalogAdapter } from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import {
  exactSourceExtension,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  completionSession,
  installCompletion,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import { loraCompletionOptions } from '../../../../../../frontend/sugarprompt/editor/src/lora-completion-options.ts';
import {
  completionAcceptanceTransaction,
  completionOptionTransaction,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-acceptance.ts';

/** Prepare actual model proposals for one source and caret. */
function fixture(
  source: string,
  caret = 12,
  kind: PromptDocumentKind = 'prompt',
  names: readonly string[] = ['first', 'second'],
) {
  const encoded = encodeSource(source);
  let state = EditorState.create({
    doc: Text.of(encoded.logical.split('\n')),
    selection: { anchor: caret },
    extensions: [
      exactSourceExtension(source),
      documentKind.of(kind),
      editorFreshness,
      completionSession,
      semanticStructures,
      history(),
    ],
  });
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(state),
    }),
  }).state;
  const scope = prepareCompletionScope(state)!,
    query = scope.target.scope.loraQueries.queryAt(scope.target.decodedRange.from, false)!;
  const adapter = createLoraCatalogAdapter(
    {
      revision: 'full-name-field',
      authoritative: true,
      items: names.map((name) => ({
        model: {
          promptName: `folder/${name}`,
          backendValue: `folder/${name}.pt`,
          basename: name,
        },
        title: name,
        subtitle: 'rea',
        triggerWords: [],
        thumbnail: { status: 'unavailable' },
        modelPageUrl: null,
      })),
    },
    { searchMetadata: [] },
  );
  const result = loraCompletionOptions(state, scope, query, caret, adapter.completionCatalog!)!;
  assert.ok(completionAcceptanceTransaction(state, result.options.at(-1)!, 'keyboard'));
  const edit = result.options.at(-1)!.prepareEdit(state, 'keyboard')!;
  const ready = edit.transactions[0]!;
  state = state.update({ effects: installCompletion.of(result) }).state;
  return { state, result, ready, edit };
}

test('forward deletion stales the visible rows; fresh rows accept the whole token, metadata and undo order', () => {
  for (const [source, removedLength, refined, accepted] of [
    ['🙂\r\n<lora:reaZ\rtail', 1, '🙂\r\n<lora:rea\rtail', '🙂\r\n<lora:folder/second:1.00>\rtail'],
    [
      '🙂\r\n<lora:rea🙂 (draft): +0.80: +0.60:IN=0.20>\rtail',
      2,
      '🙂\r\n<lora:rea (draft): +0.80: +0.60:IN=0.20>\rtail',
      '🙂\r\n<lora:folder/second: +0.80: +0.60:IN=0.20>\rtail',
    ],
  ] as const) {
    const original = fixture(source);
    const deleted = original.state.update({
      changes: { from: 12, to: 12 + removedLength },
      userEvent: 'delete.forward',
    }).state;
    assert.equal(deleted.field(completionSession)!.result, original.result);
    assert.equal(completionOptionTransaction(deleted, original.result, 1, 'keyboard'), null);
    const fresh = fixture(refined);
    const transaction = completionOptionTransaction(fresh.state, fresh.result, 1, 'keyboard')!;
    assert.ok(transaction);
    let state = transaction.state;
    assert.equal(readExactSource(state) === accepted, true);
    state = state.update({
      changes: { from: state.selection.main.head, insert: 'X' },
      userEvent: 'input.type',
    }).state;
    assert.equal(readExactSource(state) === accepted.replace('>', '>X'), true);
    undo({
      state,
      dispatch: (value) => {
        state = value.state;
      },
    });
    assert.equal(readExactSource(state) === accepted, true);
    undo({
      state,
      dispatch: (value) => {
        state = value.state;
      },
    });
    assert.equal(readExactSource(state) === refined, true);
    assert.equal(state.selection.main.head, 12);
  }
});
test('name continuation rejects removal of scheduling delimiters, marker text and external source', () => {
  const source = '🙂\r\n<lora:rea: +0.80: +0.60:IN=0.20>\rtail';
  const original = fixture(source);
  for (const [from, to] of [
    [12, 13],
    [8, 9],
    [0, 2],
  ] as const) {
    const state = original.state.update({
      changes: { from, to },
      userEvent: 'delete.forward',
    }).state;
    assert.equal(state.field(completionSession), null);
    assert.equal(completionOptionTransaction(state, original.result, 1, 'keyboard'), null);
  }
});

test('a real CSV LoRA query retains a literal quoted canonical name and exact schedule suffix inside its value', () => {
  const source = 'header\n"<lora:rea: +0.80:IN=0.20>",tail',
    prepared = fixture(source, 17, 'wildcard-csv', ['literal"model']),
    option = prepared.result.options[0]!;
  assert.equal(option.kind, 'lora');
  assert.equal(option.label, 'literal"model');
  assert.equal(prepared.result.from, 8);
  assert.equal(prepared.result.query.contentFrom, 14);
  assert.equal(prepared.result.query.scope.mapping.text, '<lora:rea: +0.80:IN=0.20>');
  assert.equal(
    readExactSource(prepared.ready.state),
    'header\n"<lora:folder/literal""model: +0.80:IN=0.20>",tail',
  );
  const followup = prepareNativeStructure(prepared.ready.state, prepared.edit);
  assert.equal(
    followup.scopes.scopes[0]!.mapping.text,
    '<lora:folder/literal"model: +0.80:IN=0.20>',
  );
  assert.equal(followup.scopes.scopes[1]!.mapping.text, 'tail');
});
