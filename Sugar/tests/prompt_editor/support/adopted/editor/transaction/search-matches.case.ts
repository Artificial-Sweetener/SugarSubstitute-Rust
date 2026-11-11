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
/** Actual CodeMirror state proves exact boundaries, freshness retirement and metadata-only history. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, Transaction } from '@codemirror/state';
import { history, undo, undoDepth } from '@codemirror/commands';
import { encodeSource } from '../../../../../../frontend/sugarprompt/core/src/line-endings.ts';
import {
  exactSourceExtension,
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  installSearchMatches,
  prepareSearchMatches,
  searchMatches,
} from '../../../../../../frontend/sugarprompt/editor/src/search-matches.ts';
import type { SearchResults } from '../../../../../../frontend/sugarprompt/services/src/search/contracts.ts';

/** Construct the cheapest real owners for exact source, freshness, native history and search publication. */
function state(source: string): EditorState {
  return EditorState.create({
    doc: encodeSource(source).logical,
    extensions: [exactSourceExtension(source), editorFreshness, searchMatches, history()],
  });
}
/** Independent explicit ranges identify authored text after a mixed newline boundary. */
function results(): SearchResults {
  return {
    request: {
      fieldId: 'field',
      queryId: 'query',
      sourceRevision: 0,
      identity: Symbol(),
    },
    resultRevision: 1,
    matches: [
      { matchId: 'a', from: 0, to: 2 },
      { matchId: 'b', from: 4, to: 6 },
    ],
    activeIndex: 1,
  };
}

test('CRLF conversion captures immutable ranges and rejects interior, invalid and ambiguous identities', () => {
  const current = state('ab\r\ncd'),
    publication = results();
  const prepared = prepareSearchMatches(current, publication);
  assert.deepEqual(prepared.ranges, [
    { from: 0, to: 2 },
    { from: 3, to: 5 },
  ]);
  assert.notEqual(prepared.results.matches, publication.matches);
  for (const matches of [
    [{ matchId: 'x', from: 3, to: 4 }],
    [{ matchId: 'x', from: 4, to: 7 }],
    [{ matchId: 'x', from: 4, to: 4 }],
    [{ matchId: 'x', from: 5, to: 4 }],
    [
      { matchId: 'x', from: 0, to: 1 },
      { matchId: 'x', from: 4, to: 5 },
    ],
  ])
    assert.throws(() =>
      prepareSearchMatches(current, {
        ...publication,
        matches,
        activeIndex: null,
      }),
    );
});

test('nonBMP units remain exact UTF16 and newline spelling-only metadata retires ranges with unchanged logical doc', () => {
  let current = state('🙂\r\nA');
  const prepared = prepareSearchMatches(current, {
    ...results(),
    matches: [
      { matchId: 'emoji', from: 0, to: 2 },
      { matchId: 'letter', from: 4, to: 5 },
    ],
    activeIndex: 1,
  });
  assert.deepEqual(prepared.ranges, [
    { from: 0, to: 2 },
    { from: 3, to: 4 },
  ]);
  current = current.update({
    effects: installSearchMatches.of(prepared),
  }).state;
  const spelling = exactReplacement(current, 2, 4, '\n');
  const transaction = current.update({ effects: spelling.effects });
  assert.equal(transaction.docChanged, false);
  assert.equal(transaction.state.doc, current.doc);
  assert.equal(readExactSource(transaction.state), '🙂\nA');
  assert.equal(transaction.state.field(searchMatches), null);
  assert.notEqual(transaction.state.field(editorFreshness).source, prepared.freshness.source);
});

test('selection retains actual prepared output; source edit, undo, baseline and context retire obsolete paint', () => {
  let current = state('ab\r\ncd');
  const prepared = prepareSearchMatches(current, results());
  current = current.update({
    effects: installSearchMatches.of(prepared),
    annotations: Transaction.addToHistory.of(false),
  }).state;
  assert.equal(undoDepth(current), 0);
  const selected = current.update({ selection: { anchor: 4 } }).state;
  assert.equal(selected.field(searchMatches), prepared);
  assert.equal(readExactSource(selected), 'ab\r\ncd');
  current = selected.update(exactReplacement(selected, 0, 1, 'z')).state;
  assert.equal(current.field(searchMatches), null);
  undo({
    state: current,
    dispatch: (transaction) => {
      current = transaction.state;
    },
  });
  assert.equal(readExactSource(current), 'ab\r\ncd');
  for (const retired of [
    current,
    state('ab\r\ncd'),
    selected.update({ effects: invalidateCommandContext.of(null) }).state,
  ])
    assert.equal(
      retired.update({ effects: installSearchMatches.of(prepared) }).state.field(searchMatches),
      null,
    );
  assert.equal(
    selected.update({ effects: installSearchMatches.of(null) }).state.field(searchMatches),
    null,
  );
});
