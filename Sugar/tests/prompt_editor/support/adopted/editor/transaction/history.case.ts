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
/** Native transaction and command proof owns bounded selection inversion, batch publication and history reuse. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, Transaction } from '@codemirror/state';
import { undo } from '@codemirror/commands';
import { historyTransactions } from '../../../../../../frontend/sugarprompt/editor/src/prompt-history.ts';
import { HistoryFixture } from './history-fixture.ts';

test('one native word group retains one selection inversion effect rather than one effect per character', () => {
  const fixture = new HistoryFixture('tail');
  fixture.select(4, 0);
  for (let index = 0; index < 1_000; index++) fixture.type('a');
  assert.equal(fixture.snapshot().undoDepth, 1);
  let effects = -1;
  assert.equal(
    undo({
      state: fixture.state,
      dispatch(transaction) {
        effects = transaction.effects.length;
        fixture.dispatch(transaction);
      },
    }),
    true,
  );
  assert.equal(effects, 1);
  assert.equal(fixture.snapshot().source === 'tail', true);
  assert.deepEqual([fixture.snapshot().anchor, fixture.snapshot().head], [4, 0]);
});

test('actual associated reverse selection and post-edit caret survive repeated native Undo and Redo with exact mixed newlines', () => {
  const source = '🙂\r\nold\rtail\n',
    fixture = new HistoryFixture(source);
  fixture.select(7, 4);
  for (const character of 'next') fixture.type(character);
  const changed = source.replace('old', 'next');
  for (let cycle = 0; cycle < 4; cycle++) {
    assert.equal(fixture.restore('undo'), true);
    const original = fixture.snapshot();
    assert.equal(original.source === source, true);
    assert.deepEqual(
      [original.anchor, original.head, original.undoDepth, original.redoDepth],
      [7, 4, 0, 1],
    );
    assert.equal(fixture.restore('redo'), true);
    const restored = fixture.snapshot();
    assert.equal(restored.source === changed, true);
    assert.deepEqual(
      [restored.anchor, restored.head, restored.undoDepth, restored.redoDepth],
      [8, 8, 1, 0],
    );
  }
});

test('ordinary transaction batches reuse their actual input array without an additional selection transaction', () => {
  const fixture = new HistoryFixture('');
  const transactions = [
    fixture.state.update({
      changes: { from: 0, insert: 'word' },
      selection: EditorSelection.cursor(4),
    }),
  ];
  assert.equal(historyTransactions(transactions), transactions);
});

test('later explicit native selection intent wins over an earlier history-owned restoration in the same batch', () => {
  const fixture = new HistoryFixture('old tail');
  fixture.select(3, 0);
  for (const character of 'next') fixture.type(character);
  const transactions: Transaction[] = [];
  undo({
    state: fixture.state,
    dispatch(transaction) {
      transactions.push(transaction);
    },
  });
  const restored = transactions[0]!;
  transactions.push(
    restored.state.update({
      selection: EditorSelection.cursor(8),
      userEvent: 'select.pointer',
    }),
  );
  assert.equal(historyTransactions(transactions), transactions);
  assert.equal(transactions.at(-1)!.newSelection.main.head, 8);
});

test('an explicit command boundary is source neutral and a new edit after Undo retires native Redo', () => {
  const fixture = new HistoryFixture('');
  fixture.type('a');
  const first = fixture.snapshot();
  fixture.boundary();
  assert.deepEqual(fixture.snapshot(), first);
  fixture.type('b');
  assert.equal(fixture.snapshot().undoDepth, 2);
  fixture.restore('undo');
  assert.equal(fixture.snapshot().source === 'a', true);
  assert.equal(fixture.snapshot().redoDepth, 1);
  fixture.type('c');
  assert.equal(fixture.snapshot().source === 'ac', true);
  assert.equal(fixture.snapshot().redoDepth, 0);
  assert.equal(fixture.restore('redo'), false);
});
