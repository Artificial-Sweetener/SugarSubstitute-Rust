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
/** Real CodeMirror state proves explicit reveal freshness, bounded lifetime, prepared-output reuse and native history metadata. */
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, Transaction, type EditorState, type StateCommand } from '@codemirror/state';
import { undo, redo, undoDepth } from '@codemirror/commands';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { prepareNativeStructure } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import {
  installProjection,
  revealProjectionToken,
  richProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { projectionState, projectionRanges } from './projection-state.ts';

/** Publish the actual semantic owner's current projection, outside the transaction callback. */
function prepared(state: EditorState): EditorState {
  return state.update({
    effects: installProjection.of(prepareProjection(state, prepareNativeStructure(state))),
  }).state;
}

/** Capture one current logical-document UTF-16 token and native source selection, without another text owner. */
function revealed(state: EditorState, from: number, to: number): EditorState {
  return state.update({
    selection: { anchor: from, head: to },
    effects: revealProjectionToken.of({
      sourceIdentity: state.field(editorFreshness).source,
      range: { from, to },
    }),
    userEvent: 'select.structural',
  }).state;
}

/** Apply an actual native history command through state dispatch; no fixture history is synthesized. */
function historyStep(state: EditorState, command: StateCommand = undo): EditorState {
  let restored = state;
  assert.equal(
    command({
      state,
      /** Retain only the state published by CodeMirror's real history command. */
      dispatch(transaction) {
        restored = transaction.state;
      },
    }),
    true,
  );
  return restored;
}

test('explicit outer reveal includes nested wrappers and reuses real prepared output until selection leaves', () => {
  const source = '((cat:1.20):1.30), tail';
  let state = prepared(projectionState(source));
  const projection = state.field(richProjection);
  const to = source.indexOf(',');
  state = revealed(state, 0, to);
  assert.deepEqual(projectionRanges(state), []);
  assert.equal(state.field(richProjection).all, projection.all);
  assert.equal(state.field(richProjection).tokens, projection.tokens);
  assert.equal(undoDepth(state), 0);
  const exposed = state.field(richProjection);
  state = state.update({ selection: { anchor: 3 } }).state;
  assert.equal(state.field(richProjection), exposed);
  state = state.update({ effects: installProjection.of(projection) }).state;
  assert.equal(state.field(richProjection), exposed);
  assert.equal(state.field(richProjection).all, projection.all);
  assert.deepEqual(projectionRanges(state), []);
  state = state.update({ selection: { anchor: source.length } }).state;
  assert.equal(state.field(richProjection).revealed, null);
  assert.equal(state.field(richProjection).decorations, projection.all);
  assert.equal(readExactSource(state) === source, true);
});

test('a stale reveal cannot adopt a same-spelling replacement or an unregistered range', () => {
  let state = prepared(projectionState('(cat:1.20)'));
  const intent = revealProjectionToken.of({
    sourceIdentity: state.field(editorFreshness).source,
    range: { from: 0, to: 10 },
  });
  state = state.update(exactReplacement(state, 0, 10, '(dog:1.20)')).state;
  state = prepared(state);
  const current = state.field(richProjection);
  state = state.update({
    effects: intent,
    selection: { anchor: 0, head: 10 },
  }).state;
  assert.equal(state.field(richProjection), current);
  state = state.update({
    effects: revealProjectionToken.of({
      sourceIdentity: state.field(editorFreshness).source,
      range: { from: 0, to: 9 },
    }),
  }).state;
  assert.equal(state.field(richProjection), current);
  assert.equal(state.field(richProjection).revealed, null);
});

test('native authoring maps reveal bounds across preparation while exact host editing retires them', () => {
  let state = revealed(prepared(projectionState('(cat:1.20), tail')), 0, 10);
  state = state.update({
    selection: { anchor: 2 },
  }).state;
  state = state.update({
    changes: { from: 2, insert: '🙂' },
    selection: { anchor: 4 },
    userEvent: 'input.type',
  }).state;
  assert.deepEqual(state.field(richProjection).revealed, { from: 0, to: 12 });
  state = prepared(state);
  assert.deepEqual(projectionRanges(state), []);
  state = state.update(exactReplacement(state, 1, 1, 'X')).state;
  state = prepared(state);
  assert.equal(state.field(richProjection).revealed, null);
  assert.equal(projectionRanges(state).length, 2);
});

test('native undo restores reveal with exact CRLF metadata, while raw mode excludes restoration', () => {
  const presentation = new Compartment();
  const source = 'lead\r\n(cat:1.20)';
  let state = revealed(prepared(projectionState(source, presentation)), 5, 15);
  state = state.update({
    changes: { from: 5, to: 15 },
    selection: { anchor: 5 },
    userEvent: 'delete.selection',
  }).state;
  assert.equal(state.field(richProjection).revealed, null);
  state = historyStep(state);
  state = prepared(state);
  assert.equal(readExactSource(state) === source, true);
  assert.deepEqual(state.field(richProjection).revealed, { from: 5, to: 15 });
  assert.deepEqual(projectionRanges(state), []);
  state = state.update(exactReplacement(state, 4, 6, '\n')).state;
  assert.equal(state.field(richProjection).revealed, null);
  state = historyStep(state);
  state = prepared(state);
  assert.equal(readExactSource(state) === source, true);
  assert.deepEqual(state.field(richProjection).revealed, { from: 5, to: 15 });
  state = state.update({
    changes: { from: 5, to: 15 },
    selection: { anchor: 5 },
    userEvent: 'delete.selection',
  }).state;
  state = state.update({
    effects: presentation.reconfigure(richRendering.of(false)),
  }).state;
  state = historyStep(state);
  assert.equal(state.field(richProjection).revealed, null);
  state = state.update({
    effects: presentation.reconfigure(richRendering.of(true)),
  }).state;
  state = prepared(state);
  assert.equal(projectionRanges(state).length, 2);
});

test('native history rebases revealed bounds across an insertion excluded from history', () => {
  const source = 'lead\r\n(cat:1.20), tail';
  let state = revealed(prepared(projectionState(source)), 5, 15);
  state = state.update({
    changes: { from: 5, to: 15 },
    selection: { anchor: 5 },
    userEvent: 'delete.selection',
  }).state;
  state = state.update({
    changes: { from: 0, insert: '🙂' },
    annotations: Transaction.addToHistory.of(false),
  }).state;
  state = prepared(historyStep(state));
  assert.equal(readExactSource(state) === '🙂' + source, true);
  assert.deepEqual(state.field(richProjection).revealed, { from: 7, to: 17 });
  assert.deepEqual([state.selection.main.anchor, state.selection.main.head], [7, 17]);
  assert.deepEqual(projectionRanges(state), []);
  state = prepared(historyStep(state, redo));
  assert.equal(readExactSource(state) === '🙂lead\r\n, tail', true);
  assert.equal(state.field(richProjection).revealed, null);
  state = prepared(historyStep(state));
  assert.deepEqual(state.field(richProjection).revealed, { from: 7, to: 17 });
  assert.deepEqual(projectionRanges(state), []);
});

test('joined native edits restore the original revealed bounds and redo their mapped range', () => {
  const source = 'lead\r\n(cat:1.20), tail';
  let state = revealed(prepared(projectionState(source)), 5, 15);
  for (const [from, insert, time] of [
    [6, 'X', 1000],
    [7, '🙂', 1100],
  ] as const)
    state = state.update({
      changes: { from, insert },
      selection: { anchor: from + insert.length },
      annotations: Transaction.time.of(time),
      userEvent: 'input.type',
    }).state;
  assert.equal(undoDepth(state), 1);
  state = prepared(historyStep(state));
  assert.equal(readExactSource(state) === source, true);
  assert.deepEqual(state.field(richProjection).revealed, { from: 5, to: 15 });
  assert.deepEqual(projectionRanges(state), []);
  state = prepared(historyStep(state, redo));
  assert.equal(readExactSource(state) === 'lead\r\n(X🙂cat:1.20), tail', true);
  assert.deepEqual(state.field(richProjection).revealed, { from: 5, to: 18 });
  assert.deepEqual(projectionRanges(state), []);
});

test('structured documents keep scenes literal and exclude malformed CSV and syntax-shaped headers', () => {
  for (const source of [
    '"(header:9)","{header}"',
    'left,right\n"(cat:2)',
    'left,right\n"  ","  "',
  ]) {
    const state = projectionState(source, new Compartment(), 'wildcard-csv');
    const prepared = prepareProjection(state, prepareNativeStructure(state));
    assert.equal(prepared.all.size, 0);
    assert.equal(prepared.tokens.size, 0);
  }
  const source = 'left,right\n"** (cat:2)\n{animal}","{animal}"';
  const presentation = new Compartment();
  let state = projectionState(source, presentation, 'wildcard-csv');
  const canonical = prepareNativeStructure(state),
    prepared = prepareProjection(state, canonical);
  const classes: string[] = [];
  prepared.all.between(0, state.doc.length, (_from, _to, value) => {
    if (value.spec.class) classes.push(value.spec.class);
  });
  assert.equal(
    classes.some((name) => name.startsWith('sp-scene')),
    false,
  );
  assert.equal(prepared.tokens.size, 3);
  state = state.update({
    effects: presentation.reconfigure(richRendering.of(false)),
  }).state;
  const raw = prepareProjection(state, canonical);
  assert.equal(raw.all.size, 0);
  assert.equal(raw.wildcardDiagnostics.size, 0);
  assert.equal(raw.allAtomic.size, 0);
  assert.deepEqual(Object.keys(raw.regionHovers), []);
});

test('CSV explicit reveal exposes one mapped value token while adjacent value and storage remain native', () => {
  const source = 'left,right\r\n"🙂(cat:2)","(dog:3)"';
  let state = projectionState(source, new Compartment(), 'wildcard-csv');
  const canonical = prepareNativeStructure(state),
    projection = prepareProjection(state, canonical);
  state = state.update({
    effects: installProjection.of(projection),
    annotations: Transaction.addToHistory.of(false),
  }).state;
  const wrappers: number[][] = [];
  projection.allAtomic.between(0, state.doc.length, (from, to) => {
    wrappers.push([from, to]);
  });
  assert.deepEqual(wrappers, [
    [14, 15],
    [18, 21],
    [24, 25],
    [28, 31],
  ]);
  state = revealed(state, 14, 21);
  const current = state.field(richProjection);
  const atomic: number[][] = [];
  current.atomic.between(0, state.doc.length, (from, to) => {
    atomic.push([from, to]);
  });
  assert.deepEqual(atomic, [
    [24, 25],
    [28, 31],
  ]);
  assert.equal(current.tokens, projection.tokens);
  assert.equal(readExactSource(state), source);
  assert.deepEqual([state.selection.main.anchor, state.selection.main.head], [14, 21]);
  assert.equal(undoDepth(state), 0);
});
