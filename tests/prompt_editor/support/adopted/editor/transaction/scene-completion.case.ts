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
/** Actual native states prove literal title replacement, retained prepared output and source/context retirement. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorState, StateEffect, Text, type Transaction } from '@codemirror/state';
import { history, undo, redo, undoDepth } from '@codemirror/commands';
import {
  encodeSource,
  sourceToDocument,
} from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { prepareSceneTitleCatalog } from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import {
  exactSourceExtension,
  readExactSource,
  lineEndings,
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
import {
  sceneTitleCatalog,
  type PreparedCompletionResult,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-model.ts';
import { prepareCompletionOptions } from '../../../../../../frontend/sugarprompt/editor/src/completion-options.ts';
import { completionOptionTransaction } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-acceptance.ts';

/** Mount real preparation and native history; input caret is exact-source UTF-16. */
function prepared(
  source = '🙂\r\n \t**Ca \t\rbody',
  sourceCaret = 10,
): {
  readonly state: EditorState;
  readonly result: PreparedCompletionResult;
  readonly catalog: Compartment;
} {
  const catalog = new Compartment(),
    encoded = encodeSource(source);
  let state = EditorState.create({
    doc: Text.of(encoded.logical.split('\n')),
    selection: {
      anchor: sourceToDocument(encoded.logical, encoded.endings, sourceCaret),
    },
    extensions: [
      exactSourceExtension(source),
      authorshipExtension,
      editorFreshness,
      semanticStructures,
      completionSession,
      history(),
      catalog.of(
        sceneTitleCatalog.of(
          prepareSceneTitleCatalog({
            status: 'ready',
            revision: 'fixture',
            titles: ['Cafe_(two:1.20)', 'Cafe close'],
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

test('keyboard and pointer accept only the literal title, retain right whitespace and exact endings with one native history event', () => {
  for (const activation of ['keyboard', 'pointer'] as const) {
    const fixture = prepared();
    let state = completionOptionTransaction(fixture.state, fixture.result, 0, activation)!.state;
    assert.equal(readExactSource(state) === '🙂\r\n \t**Cafe_(two:1.20) \t\rbody', true);
    assert.equal(state.selection.main.head, 7 + 'Cafe_(two:1.20)'.length);
    assert.deepEqual(state.field(lineEndings), fixture.state.field(lineEndings));
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
    assert.equal(readExactSource(state) === '🙂\r\n \t**Ca \t\rbody', true);
    assert.equal(redo(target), true);
    assert.equal(readExactSource(state) === '🙂\r\n \t**Cafe_(two:1.20) \t\rbody', true);
  }
});

test('title refinement stales the visible row and fresh rows consume the complete current prefix', () => {
  const fixture = prepared(),
    caret = fixture.state.selection.main.head;
  const state = fixture.state.update({
    changes: { from: caret, insert: 'f' },
    selection: { anchor: caret + 1 },
    userEvent: 'input.type',
  }).state;
  assert.equal(state.field(semanticStructures).native, null);
  assert.equal(state.field(completionSession)!.result === fixture.result, true);
  assert.equal(completionOptionTransaction(state, fixture.result, 0, 'keyboard'), null);
  const fresh = prepared('🙂\r\n \t**Caf \t\rbody', 11);
  const accepted = completionOptionTransaction(fresh.state, fresh.result, 0, 'keyboard')!;
  assert.ok(accepted);
  assert.equal(readExactSource(accepted.state) === '🙂\r\n \t**Cafe_(two:1.20) \t\rbody', true);
});

test('line controls, marker/source edits, selection, composition, permission and metadata changes reject stale prepared titles', () => {
  const fixture = prepared(),
    caret = fixture.state.selection.main.head;
  for (const spec of [
    ...['\n', '\u0085', '\u2028', '\0'].map((insert) => ({
      changes: { from: caret, insert },
      selection: { anchor: caret + insert.length },
      userEvent: 'input.type',
    })),
    {
      changes: { from: caret, insert: 'f' },
      selection: { anchor: caret + 1 },
      userEvent: 'input.type.compose',
    },
    { changes: { from: 4, to: 8, insert: 'x' }, userEvent: 'input.type' },
    {
      changes: { from: 0, insert: 'x' },
      selection: { anchor: caret + 1 },
      userEvent: 'input.type',
    },
    { selection: { anchor: caret - 1, head: caret } },
    { effects: StateEffect.appendConfig.of(EditorState.readOnly.of(true)) },
    {
      effects: [
        fixture.catalog.reconfigure(
          sceneTitleCatalog.of(
            prepareSceneTitleCatalog({
              status: 'pending',
              revision: 'pending',
            }),
          ),
        ),
        invalidateCommandContext.of(null),
      ],
    },
  ])
    assert.equal(
      completionOptionTransaction(fixture.state.update(spec).state, fixture.result, 0, 'keyboard'),
      null,
    );
});

test('TXT and CSV values never acquire prompt scene-title completion even with a populated workflow catalog', () => {
  const catalog = prepareSceneTitleCatalog({
    status: 'ready',
    revision: 'values',
    titles: ['Cafe close'],
  });
  assert.equal(catalog.search('Ca').length, 1);
  for (const [kind, source, caret] of [
    ['wildcard-txt', '**Ca', 4],
    ['wildcard-csv', 'header\n"**Ca",tail', 12],
  ] as const) {
    let state = EditorState.create({
      doc: source,
      selection: { anchor: caret },
      extensions: [
        exactSourceExtension(source),
        editorFreshness,
        semanticStructures,
        documentKind.of(kind),
        sceneTitleCatalog.of(catalog),
      ],
    });
    state = state.update({
      effects: installSemanticStructure.of({
        purpose: 'native',
        prepared: prepareNativeStructure(state),
      }),
    }).state;
    assert.equal(state.field(semanticStructures).native!.scopes.scopes[0]!.sceneQueries, null);
    assert.equal(prepareCompletionOptions(state), null);
    assert.equal(readExactSource(state), source);
    assert.equal(state.selection.main.head, caret);
  }
});
