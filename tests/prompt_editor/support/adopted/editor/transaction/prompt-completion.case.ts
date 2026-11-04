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
/** Real native states/history independently prove prepared completion source/context/caret freshness. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect, Text } from '@codemirror/state';
import { history, undo, redo } from '@codemirror/commands';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import { preparedTagCompletion } from './tag-completion-fixture.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { prepareCompletionOptions } from '../../../../../../frontend/sugarprompt/editor/src/completion-options.ts';
import { tagCompletionCatalog } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-model.ts';
import {
  createLoraCatalogAdapter,
  prepareTagCompletionCatalog,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import { loraCompletionOptions } from '../../../../../../frontend/sugarprompt/editor/src/lora-completion-options.ts';
import {
  exactSourceExtension,
  exactReplacement,
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
import { prepareCompletionScope } from '../../../../../../frontend/sugarprompt/editor/src/completion-splice.ts';
import type { PreparedCompletion } from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-model.ts';
import {
  completionAcceptanceTransaction,
  completionOptionTransaction,
} from '../../../../../../frontend/sugarprompt/editor/src/prompt-completion-acceptance.ts';
import {
  completionSession,
  installCompletion,
  requestCompletionAcceptance,
} from '../../../../../../frontend/sugarprompt/editor/src/completion-session.ts';

/** Capture a real prepared source transformation with current native epochs, not a reconstructed acceptance implementation. */
function prepared(): { state: EditorState; completion: PreparedCompletion } {
  const fixture = preparedTagCompletion('🙂\r\nsoft ght', 8);
  return { state: fixture.state, completion: fixture.result.options[0]! };
}

test('tag acceptance preserves authored whitespace before the replacement for both activation paths', () => {
  for (const prefix of ['cat, ', 'cat,  ', 'cat,\t', 'cat,\r\n  ', 'cat,\u00a0', 'cat,\u2009']) {
    for (const activation of ['keyboard', 'pointer'] as const) {
      const source = prefix + 'so';
      const { state, result } = preparedTagCompletion(source, encodeSource(source).logical.length);
      const accepted = completionOptionTransaction(state, result, 0, activation);
      assert.ok(accepted);
      assert.equal(
        readExactSource(accepted.state),
        prefix + 'soft light' + (activation === 'keyboard' ? ', ' : ''),
      );
    }
  }
});

test('prepared completion changes exact source/caret in one native event and history restores existing suffix and mixed endings', () => {
  const fixture = prepared();
  let state = fixture.state;
  const accepted = completionAcceptanceTransaction(state, fixture.completion, 'keyboard');
  assert.ok(accepted);
  state = accepted.state;
  assert.equal(readExactSource(state), '🙂\r\nsoft light, ');
  assert.equal(state.selection.main.head, 15);
  assert.equal(
    undo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(state), '🙂\r\nsoft ght');
  assert.equal(state.selection.main.head, 8);
  assert.equal(
    redo({
      state,
      dispatch: (transaction) => {
        state = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(state), '🙂\r\nsoft light, ');
});

test('native model options accept with scheduler suffixes, stale on name refinement and isolate following typing', () => {
  const source = 'ß\r\n<lora:rea:0.80:0.60:IN=0.2>\rtail';
  let state = EditorState.create({
    doc: Text.of(encodeSource(source).logical.split('\n')),
    selection: { anchor: 11 },
    extensions: [
      exactSourceExtension(source),
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
      revision: 'native-options',
      authoritative: true,
      items: ['first', 'second'].map((name) => ({
        model: {
          promptName: `folder/${name}`,
          backendValue: `folder/${name}.safetensors`,
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
  const result = loraCompletionOptions(state, scope, query, 11, adapter.completionCatalog!)!;
  assert.equal(result.options.length, 2);
  const firstEdit = completionAcceptanceTransaction(state, result.options[0]!, 'keyboard')!;
  const secondEdit = completionAcceptanceTransaction(state, result.options[1]!, 'keyboard')!;
  const pointer = completionAcceptanceTransaction(state, result.options[1]!, 'pointer')!;
  assert.ok(Array.isArray(firstEdit.effects) && Array.isArray(secondEdit.effects));
  assert.equal(firstEdit.effects[0]!.value, secondEdit.effects[0]!.value);
  // Model names carry no trailing delimiter, so both activations accept the same text.
  assert.equal(pointer.newDoc.eq(secondEdit.newDoc), true);
  state = state.update({ effects: installCompletion.of(result) }).state;
  const installed = state;
  state = state.update({
    changes: { from: 11, insert: "l (v2), [draft] {stage} 'quoted'" },
    selection: { anchor: 43 },
    userEvent: 'input.type',
  }).state;
  assert.equal(completionAcceptanceTransaction(state, result.options[1]!, 'keyboard'), null);
  assert.equal(completionOptionTransaction(state, result, 1, 'keyboard'), null);
  // No fresh row matches the refined name, so nothing is accepted for it; the installed rows
  // still accept their own query.
  state = state.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(state),
    }),
  }).state;
  const refinedScope = prepareCompletionScope(state)!;
  assert.equal(
    loraCompletionOptions(
      state,
      refinedScope,
      refinedScope.target.scope.loraQueries.queryAt(refinedScope.target.decodedRange.from, false)!,
      43,
      adapter.completionCatalog!,
    ),
    null,
  );
  state = installed;
  const transaction = completionOptionTransaction(state, result, 1, 'keyboard');
  assert.ok(transaction);
  assert.equal(transaction.newDoc.eq(secondEdit.newDoc), true);
  state = transaction.state;
  assert.equal(readExactSource(state), 'ß\r\n<lora:folder/second:0.80:0.60:IN=0.2>\rtail');
  state = state.update({
    changes: { from: state.selection.main.head, insert: 'X' },
    userEvent: 'input.type',
  }).state;
  undo({
    state,
    dispatch: (value) => {
      state = value.state;
    },
  });
  assert.equal(readExactSource(state), 'ß\r\n<lora:folder/second:0.80:0.60:IN=0.2>\rtail');
  undo({
    state,
    dispatch: (value) => {
      state = value.state;
    },
  });
  assert.equal(readExactSource(state), source);
  assert.equal(state.selection.main.head, 11);
});

test('source restoration, catalog/permission context, changed caret and directional selection cannot revive stale prepared acceptance', () => {
  const { state, completion } = prepared();
  assert.equal(
    completionAcceptanceTransaction(
      state.update({ effects: invalidateCommandContext.of(null) }).state,
      completion,
      'keyboard',
    ),
    null,
  );
  assert.equal(
    completionAcceptanceTransaction(
      state.update({ selection: { anchor: 7 } }).state,
      completion,
      'keyboard',
    ),
    null,
  );
  assert.equal(
    completionAcceptanceTransaction(
      state.update({ selection: { anchor: 8, head: 4 } }).state,
      completion,
      'keyboard',
    ),
    null,
  );
  assert.equal(
    completionAcceptanceTransaction(
      state.update({
        effects: StateEffect.reconfigure.of([
          exactSourceExtension(readExactSource(state)),
          editorFreshness,
          authorshipExtension,
          history(),
          EditorState.readOnly.of(true),
        ]),
      }).state,
      completion,
      'keyboard',
    ),
    null,
  );
  let edited = state.update(exactReplacement(state, 9, 9, 'x')).state;
  assert.equal(completionAcceptanceTransaction(edited, completion, 'keyboard'), null);
  undo({
    state: edited,
    dispatch: (transaction) => {
      edited = transaction.state;
    },
  });
  assert.equal(readExactSource(edited), readExactSource(state));
  assert.equal(completionAcceptanceTransaction(edited, completion, 'keyboard'), null);
});

test('CSV literal candidates encode quotes in actual multiline values and preserve neighboring cells with native undo', () => {
  const source = 'header\n"so\nnext",tail';
  let state = EditorState.create({
    doc: source,
    selection: { anchor: 10 },
    extensions: [
      exactSourceExtension(source),
      editorFreshness,
      semanticStructures,
      authorshipExtension,
      history(),
      documentKind.of('wildcard-csv'),
      tagCompletionCatalog.of(
        prepareTagCompletionCatalog({
          revision: 'literal',
          bundled: [],
          customTags: ['soft_"quote"'],
          censoredTags: [],
        }),
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
  assert.equal(result.options.length, 1);
  assert.equal(result.query.scope.mapping.text, 'so\nnext');
  const edit = result.options[0]!.prepareEdit(state, 'pointer')!;
  const accepted = edit.transactions[0]!;
  assert.ok(accepted);
  assert.equal(
    completionAcceptanceTransaction(state, result.options[0]!, 'pointer')!.newDoc.eq(
      accepted.newDoc,
    ),
    true,
  );
  assert.equal(accepted.isUserEvent('input.complete'), true);
  assert.equal(accepted.scrollIntoView, true);
  state = accepted.state;
  assert.equal(readExactSource(state), 'header\n"soft ""quote""\nnext",tail');
  assert.equal(state.selection.main.head, 22);
  const populated = prepareNativeStructure(state, edit);
  assert.equal(populated.scopes.scopes[0]!.mapping.text, 'soft "quote"\nnext');
  assert.equal(populated.scopes.scopes[1]!.mapping.text, 'tail');
  assert.equal(
    undo({
      state,
      dispatch(transaction) {
        state = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(state), source);
  assert.equal(state.selection.main.head, 10);
});
test('CSV physical encoding changes stale the rows and retain only a deferred actual-choice intent', () => {
  const source = 'header\n"so",tail';
  let state = EditorState.create({
    doc: source,
    selection: { anchor: 10 },
    extensions: [
      exactSourceExtension(source),
      editorFreshness,
      semanticStructures,
      completionSession,
      authorshipExtension,
      history(),
      documentKind.of('wildcard-csv'),
      tagCompletionCatalog.of(
        prepareTagCompletionCatalog({
          revision: 'encoding',
          bundled: [],
          customTags: ['soft_"quote"'],
          censoredTags: [],
        }),
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
  state = state.update({ effects: installCompletion.of(result) }).state;
  state = state.update({
    changes: { from: 10, insert: '""' },
    selection: { anchor: 12 },
    userEvent: 'input.type',
  }).state;
  assert.equal(readExactSource(state), 'header\n"so""",tail');
  assert.equal(state.field(completionSession)?.phase, 'refreshing');
  assert.equal(completionOptionTransaction(state, result, 0, 'keyboard'), null);
  state = state.update({
    effects: requestCompletionAcceptance.of({
      result,
      index: 0,
      activation: 'keyboard',
    }),
  }).state;
  assert.equal(state.field(completionSession)?.acceptance?.activation, 'keyboard');
  assert.equal(state.field(semanticStructures).native, null);
  assert.equal(prepareCompletionOptions(state), null);
  const fresh = prepareNativeStructure(state);
  assert.equal(fresh.scopes.scopes[0]!.mapping.text, 'so"');
  assert.equal(fresh.scopes.scopes[1]!.mapping.text, 'tail');
});
