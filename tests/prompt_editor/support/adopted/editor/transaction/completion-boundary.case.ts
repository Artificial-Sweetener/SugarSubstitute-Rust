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
/** Fixed query boundaries permit prepared output reuse through word separators and quoted content. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { preparedTagCompletion } from './tag-completion-fixture.ts';
import { completionOptionTransaction } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-acceptance.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { EditorState } from '@codemirror/state';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import type { PromptDocumentKind } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
  resolveCompletionQuery,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { exactSourceExtension } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  prepareCompletionScope,
  completionPosition,
  prepareScopedCompletionEdit,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-splice.ts';
import {
  prepareNativeStructure,
  installSemanticStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';

test('word separators and fixed quotes reuse actual prepared acceptance without fresh semantics', () => {
  for (const [source, removed, inserted, expected] of [
    ['soft l', 0, ' ', 'soft light'],
    ['soft l', 0, '\t', 'soft light'],
    ['soft \t', 1, '', 'soft light'],
    ['"soft l', 0, 'i', '"soft light"'],
    ["'soft l", 0, 'i', "'soft light'"],
  ] as const) {
    const { state, result } = preparedTagCompletion(source, source.length);
    const changed = state.update({
      changes: {
        from: source.length - removed,
        to: source.length,
        insert: inserted,
      },
      selection: { anchor: source.length - removed + inserted.length },
      userEvent: removed ? 'delete.backward' : 'input.type',
    }).state;
    assert.equal(changed.field(semanticStructures).native, null);
    for (const activation of ['keyboard', 'pointer'] as const) {
      const accepted = completionOptionTransaction(changed, result, 0, activation);
      assert.ok(accepted);
      assert.equal(
        readExactSource(accepted.state),
        expected + (activation === 'keyboard' ? ', ' : ''),
      );
      assert.equal(
        accepted.effects[0],
        result.options[0]!.acceptance[activation].splice!.effects[0],
      );
    }
  }
});

test('leading separator edits and changed query openers require fresh semantics', () => {
  for (const [source, from, to, insert] of [
    ['soft l', 0, 0, ' '],
    ['soft l', 0, 4, ''],
    ['"soft l', 0, 1, ''],
    ["'soft l", 0, 1, ''],
    ['soft l', 6, 6, '"'],
  ] as const) {
    const { state, result } = preparedTagCompletion(source, source.length);
    const changed = state.update({
      changes: { from, to, insert },
      selection: { anchor: source.length + insert.length - (to - from) },
      userEvent: insert ? 'input.type' : 'delete.backward',
    }).state;
    assert.equal(completionOptionTransaction(changed, result, 0, 'pointer'), null);
  }
});

test('retained complete edits carry authored continuation once and reject separator changes', () => {
  const source = 'soft lght,\t next';
  const { state, result } = preparedTagCompletion(source, 6);
  const changed = state.update({
    changes: { from: 6, insert: 'i' },
    selection: { anchor: 7 },
    userEvent: 'input.type',
  }).state;
  assert.equal(changed.field(semanticStructures).native, null);
  const accepted = completionOptionTransaction(changed, result, 0, 'keyboard');
  assert.ok(accepted);
  assert.equal(readExactSource(accepted.state) === 'soft light,\t next', true);
  assert.equal(accepted.newSelection.main.head, 13);
  assert.equal(
    accepted.effects[0],
    result.options[0]!.acceptance.keyboard.right!.splice.effects[0],
  );
  for (const [from, to, insert] of [
    [9, 10, ';'],
    [10, 11, ' '],
  ] as const) {
    const altered = state.update({
      changes: { from, to, insert },
      userEvent: 'input.type',
    }).state;
    assert.equal(completionOptionTransaction(altered, result, 0, 'keyboard'), null);
  }
});

/** Publish the actual canonical preparation for scope-boundary proof; no completion DTO or grammar is reconstructed. */
function scopedState(source: string, caret: number, kind: PromptDocumentKind): EditorState {
  const state = EditorState.create({
    doc: encodeSource(source).logical,
    selection: { anchor: caret },
    extensions: [
      exactSourceExtension(source),
      editorFreshness,
      semanticStructures,
      documentKind.of(kind),
    ],
  });
  return state.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(state),
    }),
  }).state;
}

test('canonical completion admission refuses protected CSV destinations and maps real quoted multiline decoded queries', () => {
  const source = 'header\r\n"  han\r\n""quoted""🙂  ",other',
    logical = encodeSource(source).logical,
    caret = logical.indexOf('han') + 3,
    state = scopedState(source, caret, 'wildcard-csv'),
    scope = prepareCompletionScope(state);
  assert.ok(scope);
  assert.equal(scope.target.scope.mapping.text, 'han\r\n"quoted"🙂');
  assert.equal(scope.target.decodedRange.from, 3);
  assert.strictEqual(
    scope.target.scope,
    state.field(semanticStructures).native!.scopes.byId[scope.target.scope.id],
  );
  const query = resolveCompletionQuery(
    scope.target.scope,
    resolveFeatureProfile(defaultPreferences, { mode: 'library' }),
    3,
    false,
  );
  assert.equal(query?.kind, 'tag');
  assert.equal(completionPosition(scope, 3), caret);
  assert.equal(completionPosition(scope, scope.target.scope.mapping.text.length - 1), null);
  assert.equal(scope.target.scope.sceneQueries, null);
  for (const position of [
    0,
    logical.indexOf('"  '),
    logical.indexOf('"  ') + 1,
    logical.indexOf('"  ') + 2,
    logical.indexOf('""quoted') + 1,
    logical.indexOf('",other') + 1,
  ]) {
    assert.equal(prepareCompletionScope(scopedState(source, position, 'wildcard-csv')), null);
  }
  const malformed = scopedState('header\n"unfinished', 12, 'wildcard-csv');
  assert.equal(prepareCompletionScope(malformed), null);
  const empty = scopedState('header\n,""', 7, 'wildcard-csv'),
    destination = prepareCompletionScope(empty);
  assert.ok(destination);
  assert.equal(destination.target.scope.mapping.text, '');
  assert.equal(destination.target.decodedRange.from, 0);
  const cross = state.update({
    selection: { anchor: caret, head: logical.length },
  }).state;
  assert.equal(prepareCompletionScope(cross), null);
});

test('an actual empty CSV destination encodes a literal quoted multiline completion payload and rejects reconstructed or stale authority', () => {
  const state = scopedState('header\n,""', 7, 'wildcard-csv'),
    scope = prepareCompletionScope(state)!,
    prepared = prepareScopedCompletionEdit(
      state,
      scope,
      { from: 0, to: 0 },
      'literal "quote"\n🙂',
      null,
    );
  assert.ok(prepared);
  assert.equal(prepared.splice, null);
  const transaction = prepared.edit.transactions[0]!;
  assert.equal(readExactSource(transaction.state), 'header\n"literal ""quote""\n🙂",""');
  const output = prepareNativeStructure(transaction.state, prepared.edit);
  assert.equal(output.scopes.scopes[0]!.mapping.text, 'literal "quote"\n🙂');
  assert.equal(output.scopes.scopes[1]!.mapping.text, '');
  assert.equal(output.nativeValues.entries.length, 3);
  const stale = state.update({
    changes: { from: 7, insert: 'changed' },
    userEvent: 'input.type',
  }).state;
  assert.equal(
    prepareScopedCompletionEdit(stale, scope, { from: 0, to: 0 }, 'candidate', null),
    null,
  );
  assert.equal(
    prepareScopedCompletionEdit(
      state,
      { context: scope.context, target: { ...scope.target } },
      { from: 0, to: 0 },
      'candidate',
      null,
    ),
    null,
  );
});
