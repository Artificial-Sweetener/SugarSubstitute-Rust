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
/** Real native transaction/history proof for normalized bulk ingestion and exact-source boundaries. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, Text } from '@codemirror/state';
import { history, undo, redo, undoDepth } from '@codemirror/commands';
import {
  encodeSource,
  documentToSource,
} from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import {
  prepareTagLexicon,
  type PromptDocumentKind,
} from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import {
  exactSourceExtension,
  readExactSource,
  lineEndings,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  enteredTextInsertion,
  type PreparedNativeIngestion,
} from '../../../../../../frontend/sugarprompt/editor/src/ingestion.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { documentKind } from '../../../../../../frontend/sugarprompt/editor/src/document-kind.ts';
import { authorship } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import { generatedShells } from '../../../../../../frontend/sugarprompt/editor/src/authorship-ranges.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';

import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  semanticStructures,
  initializeNativeStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';

/** Consume exact actual transactions in their prepared order without reconstructing specifications. */
function applyIngestion(plan: PreparedNativeIngestion): EditorState {
  let state = plan.startState;
  for (const transaction of plan.transactions) {
    assert.equal(transaction.startState, state);
    state = transaction.state;
  }
  return state;
}
/** Observe the actual source-changing command after its finite metadata preparation. */
function finalIngestionTransaction(
  plan: PreparedNativeIngestion,
): import('@codemirror/state').Transaction {
  applyIngestion(plan);
  const transaction = plan.transactions.at(-1);
  if (!transaction) throw new Error('Expected native ingestion transaction');
  return transaction;
}

/** Construct the actual newline/history owners with a directional native selection. */
function initial(
  source: string,
  anchor = 0,
  head = anchor,
  rich = true,
  kind: PromptDocumentKind = 'prompt',
): EditorState {
  return initializeNativeStructure(
    EditorState.create({
      doc: Text.of(encodeSource(source).logical.split('\n')),
      selection: { anchor, head },
      extensions: [
        exactSourceExtension(source),
        editorFreshness,
        semanticStructures,
        authorshipExtension,
        history(),
        richRendering.of(rich),
        documentKind.of(kind),
      ],
    }),
  );
}

test('raw ordinary insertion preserves syntax, directional undo and exact joined newline seams', () => {
  const source = 'prefix\r\nold\rtail';
  const inserted = '(cat) <lora:model:.8> vertin (reverse:1999)\r\n[SEP]';
  let state = initial(source, 10, 7, false);
  state = applyIngestion(
    enteredTextInsertion(state, 8, 11, inserted, prepareTagLexicon(['vertin (reverse:1999)'])),
  );
  assert.equal(readExactSource(state), 'prefix\r\n' + inserted + '\rtail');
  assert.equal(undoDepth(state), 1);
  undo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state), source);
  assert.deepEqual([state.selection.main.anchor, state.selection.main.head], [10, 7]);
  for (const [before, from, to, text, expected, caret] of [
    ['\ntail', 0, 0, '\r', '\r\ntail', 2],
    ['tail\r', 5, 5, '\n', 'tail\r\n', 6],
    ['(cat)', 0, 5, '(cat)', '(cat)', 5],
  ] as const) {
    const initialState = initial(before, 0, 0, false);
    const after = applyIngestion(enteredTextInsertion(initialState, from, to, text));
    assert.equal(readExactSource(after), expected);
    assert.equal(
      documentToSource(after.doc.toString(), after.field(lineEndings), after.selection.main.head),
      caret,
    );
    assert.equal(undoDepth(after), before === expected ? 0 : 1);
  }
});

test('normalization changes only entered source and restores directional selection in one undo', () => {
  const source = '(outside:.333)\r\nold\runtouched',
    lexicon = prepareTagLexicon(['vertin (reverse:1999)']);
  let state = initial(source, 18, 15);
  const transaction = finalIngestionTransaction(
    enteredTextInsertion(
      state,
      16,
      19,
      '🙂, (cat), vertin (reverse:1999), <lora:model:.8>',
      lexicon,
    ),
  );
  const expected =
    '(outside:.333)\r\n🙂, (cat:1.10), vertin \\(reverse:1999\\), <lora:model:0.80>\runtouched';
  state = transaction.state;
  assert.equal(readExactSource(state), expected);
  assert.equal(
    documentToSource(state.doc.toString(), state.field(lineEndings), state.selection.main.head),
    expected.indexOf('\runtouched'),
  );
  const changes: number[][] = [];
  transaction.changes.iterChanges((from, to) => changes.push([from, to]));
  assert.deepEqual(changes, [[15, 18]]);
  assert.equal(undoDepth(state), 1);
  undo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state), source);
  assert.equal(state.selection.main.anchor, 18);
  assert.equal(state.selection.main.head, 15);
  redo({
    state,
    dispatch: (transaction) => {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state), expected);
});

test('empty partition insertion keeps each exact newline convention and caret before its added ending', () => {
  for (const ending of ['\n', '\r', '\r\n']) {
    const source = `[SEP]${ending}[SEP]`,
      position = 5 + ending.length;
    const state = applyIngestion(
      enteredTextInsertion(initial(source, 6), position, position, '(cat)'),
    );
    const expected = `[SEP]${ending}(cat:1.10)${ending}[SEP]`;
    assert.equal(readExactSource(state), expected);
    assert.equal(
      documentToSource(state.doc.toString(), state.field(lineEndings), state.selection.main.head),
      expected.lastIndexOf(ending),
    );
  }
});

test('shared prefix/suffix astral scalars and newly joined CRLF seams retain exact source', () => {
  for (const [source, from, to, text, expected, head] of [
    ['🙂tail', 0, 2, '🙃', '🙃tail', 2],
    ['tail🙂', 4, 6, '🙃', 'tail🙃', 6],
    ['\ntail', 0, 0, '\r', '\r\ntail', 2],
    ['tail\r', 5, 5, '\n', 'tail\r\n', 6],
    ['🙂\r\n', 0, 0, '', '🙂\r\n', 0],
  ] as const) {
    const state = applyIngestion(enteredTextInsertion(initial(source), from, to, text));
    assert.equal(readExactSource(state), expected);
    assert.equal(
      documentToSource(state.doc.toString(), state.field(lineEndings), state.selection.main.head),
      head,
    );
  }
  assert.throws(() => enteredTextInsertion(initial('a\r\nb'), 2, 3, 'x'), /inside CRLF/u);
  assert.throws(() => enteredTextInsertion(initial('abc'), 2, 1, 'x'), RangeError);
  const unchanged = initial('🙂\r\n');
  const noChange = applyIngestion(enteredTextInsertion(unchanged, 0, 0, ''));
  assert.equal(undoDepth(noChange), 0);
});

test('decoded CSV ingestion normalizes one empty destination with literal quoted newlines and non-BMP text', () => {
  const state = initial('header\n,""', 7, 7, true, 'wildcard-csv');
  const plan = enteredTextInsertion(state, 7, 7, '(cat)\r\n"🙂"');
  const after = applyIngestion(plan);
  assert.equal(readExactSource(after), 'header\n"(cat:1.10)\r\n""🙂""",""');
  assert.ok(plan.edit);
  assert.equal(plan.edit.postValues.scopes.scopes[0]!.mapping.text, '(cat:1.10)\r\n"🙂"');
  assert.equal(plan.edit.postValues.scopes.scopes[1]!.mapping.text, '');
  assert.deepEqual(
    generatedShells(after.field(authorship)).map(
      ({ from, to, contentFrom, contentTo, nestingDepth }) => ({
        from,
        to,
        contentFrom,
        contentTo,
        nestingDepth,
      }),
    ),
    [{ from: 8, to: 18, contentFrom: 9, contentTo: 12, nestingDepth: 1 }],
  );
  assert.equal(undoDepth(after), 1);
  let restored = after;
  assert.equal(
    undo({
      state: after,
      dispatch: (transaction) => {
        restored = transaction.state;
      },
    }),
    true,
  );
  assert.equal(readExactSource(restored), 'header\n,""');
  assert.equal(restored.field(authorship).generated.size, 0);
});

test('header padding malformed and cross-value ordinary insertion retain exact policy without a grammar fallback', () => {
  for (const [source, from, to, expected] of [
    ['header\ncat,dog', 0, 0, '(cat)header\ncat,dog'],
    ['header\n"  dog  ",tail', 8, 8, 'header\n"(cat)  dog  ",tail'],
    ['header\n"unfinished', 12, 12, 'header\n"unfi(cat)nished'],
    ['header\ncat,dog', 8, 12, 'header\nc(cat)og'],
  ] as const) {
    const state = initial(source, from, from, true, 'wildcard-csv'),
      plan = enteredTextInsertion(state, from, to, '(cat)');
    const after = applyIngestion(plan);
    assert.equal(plan.edit, null);
    assert.equal(readExactSource(after), expected);
    assert.equal(after.field(authorship).generated.size, 0);
  }
});

test('independent CSV value ingestion maps generated shells through actual encoded post scopes', () => {
  let state = initial('header\ncat,dog', 7, 7, true, 'wildcard-csv');
  state = applyIngestion(enteredTextInsertion(state, 7, 10, '(cat)'));
  assert.equal(readExactSource(state), 'header\n(cat:1.10),dog');
  const previous = generatedShells(state.field(authorship));
  const from = readExactSource(state).indexOf('dog');
  const plan = enteredTextInsertion(state, from, from + 3, '(dog) "🙂"');
  state = applyIngestion(plan);
  assert.equal(readExactSource(state), 'header\n(cat:1.10),"(dog:1.10) ""🙂"""');
  assert.ok(plan.edit);
  assert.equal(plan.edit.postValues.scopes.scopes[0]!.mapping.text, '(cat:1.10)');
  assert.equal(plan.edit.postValues.scopes.scopes[1]!.mapping.text, '(dog:1.10) "🙂"');
  const shells = generatedShells(state.field(authorship));
  assert.equal(shells.length, 2);
  assert.deepEqual(shells[0], previous[0]);
  assert.deepEqual(shells[1], {
    from: 19,
    to: 29,
    contentFrom: 20,
    contentTo: 23,
    nestingDepth: 1,
  });
});

test('CSV known literal parentheses normalize through decoded input without changing quoted neighboring storage', () => {
  const state = initial('header\n,"neighbor ""quote"""', 7, 7, true, 'wildcard-csv');
  const plan = enteredTextInsertion(
    state,
    7,
    7,
    'vertin (reverse:1999), "🙂"\r\nend',
    prepareTagLexicon(['vertin (reverse:1999)']),
  );
  const after = applyIngestion(plan);
  assert.equal(
    readExactSource(after),
    'header\n"vertin \\(reverse:1999\\), ""🙂""\r\nend","neighbor ""quote"""',
  );
  assert.ok(plan.edit);
  assert.equal(
    plan.edit.postValues.scopes.scopes[0]!.mapping.text,
    'vertin \\(reverse:1999\\), "🙂"\r\nend',
  );
  assert.equal(plan.edit.postValues.scopes.scopes[1]!.mapping.text, 'neighbor "quote"');
  assert.equal(after.field(authorship).generated.size, 0);
});
