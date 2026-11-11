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
/** Real native states prove whole-placeholder completion, exact newline/history ownership and source-current retained output. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorState, Text, type Transaction } from '@codemirror/state';
import { history, undo, redo, undoDepth } from '@codemirror/commands';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import type { PromptDocumentKind } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { completionAcceptanceTransaction } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-acceptance.ts';
import { prepareWildcardCatalog } from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import {
  exactSourceExtension,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  semanticStructures,
  installSemanticStructure,
  prepareNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  completionSession,
  installCompletion,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';
import { wildcardCatalog } from '../../../../../../frontend/sugarprompt/editor/src/wildcard-catalog.ts';
import { prepareCompletionOptions } from '../../../../../../frontend/sugarprompt/editor/src/completion-options.ts';
import { completionOptionTransaction } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-acceptance.ts';
import type { PreparedCompletionResult } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-model.ts';

/** Mount the actual semantic preparation and completion owner; caret uses logical-document UTF-16. */
function prepared(
  source = '🙂\r\n{an  }\rtail',
  caret = 6,
  kind: PromptDocumentKind = 'prompt',
  textIdentifiers: readonly string[] = ['animals', 'animals} tail', 'animals|variant'],
): {
  readonly state: EditorState;
  readonly result: PreparedCompletionResult;
  readonly catalog: Compartment;
} {
  const catalog = new Compartment();
  let state = EditorState.create({
    doc: Text.of(encodeSource(source).logical.split('\n')),
    selection: { anchor: caret },
    extensions: [
      exactSourceExtension(source),
      documentKind.of(kind),
      authorshipExtension,
      editorFreshness,
      semanticStructures,
      completionSession,
      history(),
      catalog.of(
        wildcardCatalog.of(
          prepareWildcardCatalog({
            status: 'ready',
            revision: 'fixture',
            textIdentifiers,
            csvSources: [{ identifier: 'characters', columns: ['Name', 'Name|variant'] }],
          }),
        ),
      ),
    ],
  });
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(state),
    }),
  }).state;
  const result = prepareCompletionOptions(state)!;
  assert.ok(result?.options.length);
  state = state.update({ effects: installCompletion.of(result) }).state;
  return { state, result, catalog };
}

test('keyboard and pointer accept the complete placeholder without punctuation and preserve exact endings and native history', () => {
  for (const activation of ['keyboard', 'pointer'] as const) {
    const fixture = prepared();
    let state = completionOptionTransaction(fixture.state, fixture.result, 0, activation)!.state;
    assert.equal(readExactSource(state), '🙂\r\n{animals}\rtail');
    assert.equal(state.selection.main.head, 12);
    assert.equal(undoDepth(state), 1);
    assert.equal(state.field(completionSession), null);
    const target = {
      get state() {
        return state;
      },
      dispatch(transaction: Transaction) {
        state = transaction.state;
      },
    };
    assert.equal(undo(target), true);
    assert.equal(readExactSource(state), '🙂\r\n{an  }\rtail');
    assert.equal(redo(target), true);
    assert.equal(readExactSource(state), '🙂\r\n{animals}\rtail');
  }
});

test('content refinement stales the visible rows and fresh rows consume the complete current placeholder', () => {
  const fixture = prepared();
  const state = fixture.state.update({
    changes: { from: 6, insert: 'i' },
    selection: { anchor: 7 },
    userEvent: 'input.type',
  }).state;
  assert.equal(state.field(semanticStructures).native, null);
  assert.equal(state.field(completionSession)!.result, fixture.result);
  assert.equal(completionOptionTransaction(state, fixture.result, 0, 'keyboard'), null);
  const fresh = prepared('🙂\r\n{ani  }\rtail', 7);
  const accepted = completionOptionTransaction(fresh.state, fresh.result, 0, 'keyboard')!;
  assert.ok(accepted);
  assert.equal(readExactSource(accepted.state), '🙂\r\n{animals}\rtail');
});

test('syntax, composition, metadata context, nonempty selection and unrelated source retire prepared wildcard edits', () => {
  const fixture = prepared();
  for (const inserted of ['}', '{', ',', '\n', '\u001c']) {
    const state = fixture.state.update({
      changes: { from: 6, insert: inserted },
      selection: { anchor: 6 + inserted.length },
      userEvent: 'input.type',
    }).state;
    assert.equal(completionOptionTransaction(state, fixture.result, 0, 'keyboard'), null);
  }
  for (const spec of [
    {
      changes: { from: 6, insert: 'i' },
      selection: { anchor: 7 },
      userEvent: 'input.type.compose',
    },
    { selection: { anchor: 5, head: 6 } },
    {
      changes: { from: 0, insert: 'x' },
      selection: { anchor: 7 },
      userEvent: 'input.type',
    },
    {
      effects: [
        fixture.catalog.reconfigure(
          wildcardCatalog.of(prepareWildcardCatalog({ status: 'pending', revision: 'pending' })),
        ),
        invalidateCommandContext.of(null),
      ],
    },
  ]) {
    const state = fixture.state.update(spec).state;
    assert.equal(completionOptionTransaction(state, fixture.result, 0, 'keyboard'), null);
  }
});

test('catalog suggestions cannot introduce a group or escape their declared placeholder kind', () => {
  const simple = prepared('{', 1).result.options;
  assert.deepEqual(
    simple.map((option) => option.label),
    ['animals', 'csv:characters:Name'],
  );
  const csv = prepared('{csv:characters:', 16).result.options;
  assert.deepEqual(
    csv.map((option) => option.label),
    ['csv:characters:Name'],
  );
  assert.equal(csv[0]!.ghost, 'Name}');
});

test("a real CSV wildcard query encodes the catalog's literal quoted identifier wholly in its value", () => {
  const source = 'header\n"{qu}",tail',
    fixture = prepared(source, 11, 'wildcard-csv', ['quoted"identifier']),
    option = fixture.result.options[0]!,
    edit = option.prepareEdit(fixture.state, 'pointer')!,
    transaction = edit.transactions[0]!;
  assert.ok(completionAcceptanceTransaction(fixture.state, option, 'pointer'));
  assert.equal(option.label, 'quoted"identifier');
  assert.equal(fixture.result.from, 8);
  assert.equal(fixture.result.query.contentFrom, 9);
  assert.equal(fixture.result.query.scope.mapping.text, '{qu}');
  assert.equal(readExactSource(transaction.state), 'header\n"{quoted""identifier}",tail');
  const followup = prepareNativeStructure(transaction.state, edit);
  assert.equal(followup.scopes.scopes[0]!.mapping.text, '{quoted"identifier}');
  assert.equal(followup.scopes.scopes[1]!.mapping.text, 'tail');
});
