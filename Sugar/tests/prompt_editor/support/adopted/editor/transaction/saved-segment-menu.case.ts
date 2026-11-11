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
/** Actual CodeMirror source, selection and ingestion prove the authored capture and history contracts. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { Compartment, EditorSelection, EditorState, Transaction } from '@codemirror/state';
import { undo, undoDepth } from '@codemirror/commands';
import { structureState } from './document-structure-fixture.ts';
import {
  prepareSavedSegmentMenu,
  savedSegmentHost,
  savedSegments,
  publishSavedSegments,
  currentSavedSegmentMenu,
  type SavedSegmentHost,
} from '../../../../../../frontend/sugarprompt/editor/src/saved-segment-menu.ts';
import {
  captureSavedSegmentContext,
  prepareSavedSegmentCatalog,
} from '../../../../../../frontend/sugarprompt/services/src/saved-segments/preparation.ts';
import { enteredTextInsertion } from '../../../../../../frontend/sugarprompt/editor/src/ingestion.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
const context = captureSavedSegmentContext({
  mode: 'library',
  scopes: [{ kind: 'global', provider: null, key: 'global', label: 'Global' }],
});
const host: SavedSegmentHost = {
  context,
  adapter: {
    async read() {
      return { status: 'ready', segments: [] };
    },
    async save() {
      return { status: 'saved' };
    },
  },
  report(cause) {
    throw cause;
  },
};
test('astral exact source capture follows native UTF-16 direction and retained CRLF', () => {
  let state = structureState('😀ab\r\ntail', 'prompt', [
    savedSegments,
    savedSegmentHost.of(host),
  ]).state;
  const catalog = prepareSavedSegmentCatalog(context, []);
  state = state.update({
    selection: EditorSelection.single(6, 2),
    effects: publishSavedSegments.of({ host, status: 'ready', catalog }),
  }).state;
  const menu = prepareSavedSegmentMenu(state, 0);
  assert.equal(menu.selectedText, 'ab\r\nt');
  assert.equal(menu.from, 2);
  assert.equal(menu.to, 7);
  assert.ok(menu.saveLeaf);
  assert.equal(state.selection.main.anchor, 6);
  const next = state.update({
    effects: publishSavedSegments.of({
      host,
      status: 'unavailable',
      catalog: null,
    }),
  }).state;
  assert.equal(currentSavedSegmentMenu(next, menu), false);
});
test('selected preset replaces the captured selection with exact newlines in one native undo event', () => {
  let state = structureState('left\r\nright', 'prompt', [
    savedSegments,
    savedSegmentHost.of(host),
    richRendering.of(false),
  ]).state;
  const catalog = prepareSavedSegmentCatalog(context, [
    {
      id: 'x',
      label: '<literal>',
      text: '😀A\r\nB\rC\nD',
      associations: [context.scopes[0]],
    },
  ]);
  state = state.update({
    selection: EditorSelection.single(10, 5),
    effects: publishSavedSegments.of({ host, status: 'ready', catalog }),
  }).state;
  const before = readExactSource(state),
    menu = prepareSavedSegmentMenu(state, 0),
    leaf = [...menu.insertLeaves][0]!;
  const prepared = enteredTextInsertion(state, menu.from, menu.to, leaf.segment.text);
  for (const transaction of prepared.transactions) state = transaction.state;
  assert.equal(readExactSource(state), 'left\r\n😀A\r\nB\rC\nD');
  assert.equal(undoDepth(state), 1);
  undo({
    state,
    dispatch(transaction: Transaction) {
      state = transaction.state;
    },
  });
  assert.equal(readExactSource(state), before);
});
test('empty native selection inserts at clicked target and unavailable storage has no executable leaves', () => {
  const state = structureState('abc', 'prompt', savedSegments).state,
    menu = prepareSavedSegmentMenu(state, 2);
  assert.equal(menu.from, 2);
  assert.equal(menu.to, 2);
  assert.equal(menu.saveLeaf, null);
  assert.equal(menu.insertLeaves.size, 0);
});

test('one insert submenu contains direct literal leaves and only multiple sections receive headings', () => {
  const workflow = captureSavedSegmentContext({
    mode: 'workflow',
    scopes: [context.scopes[0], { kind: 'family', provider: 'host', key: 'f', label: '<Family>' }],
  });
  const catalog = prepareSavedSegmentCatalog(workflow, [
    {
      id: 'g',
      label: 'Global item',
      text: 'g',
      associations: [workflow.scopes[0]],
    },
    { id: 'f', label: 'Family item', text: 'f', associations: workflow.scopes },
  ]);
  const state = structureState('source', 'prompt', [
    savedSegments,
    savedSegmentHost.of(host),
  ]).state.update({
    effects: publishSavedSegments.of({ host, status: 'ready', catalog }),
  }).state;
  const submenu = prepareSavedSegmentMenu(state, 0).items[1];
  assert.equal(submenu?.kind, 'submenu');
  if (submenu?.kind !== 'submenu') assert.fail();
  assert.deepEqual(
    submenu.children.map((item) => item.kind),
    ['heading', 'insert-segment', 'separator', 'heading', 'insert-segment'],
  );
  assert.deepEqual(
    submenu.children
      .filter((item) => item.kind === 'heading')
      .map((item) => ('label' in item ? item.label : '')),
    ['<Family>', 'Global'],
  );
  const single = prepareSavedSegmentCatalog(context, [
    { id: 'g', label: 'Only', text: 'g', associations: context.scopes },
  ]);
  const next = state.update({
    effects: publishSavedSegments.of({
      host,
      status: 'ready',
      catalog: single,
    }),
  }).state;
  const only = prepareSavedSegmentMenu(next, 0).items[1];
  assert.equal(only?.kind, 'submenu');
  if (only?.kind !== 'submenu') assert.fail();
  assert.deepEqual(
    only.children.map((item) => item.kind),
    ['insert-segment'],
  );
});

test('accepted catalog rows and genuine membership are reused through native source and directional selection changes', () => {
  const catalog = prepareSavedSegmentCatalog(context, [
    {
      id: 'a',
      label: '<literal>',
      text: 'A\r\n😀',
      associations: context.scopes,
    },
  ]);
  let state = structureState('first\r\nsecond', 'prompt', [
    savedSegments,
    savedSegmentHost.of(host),
  ]).state.update({
    effects: publishSavedSegments.of({ host, status: 'ready', catalog }),
  }).state;
  const original = prepareSavedSegmentMenu(state, 0);
  const item = original.items[1];
  assert.equal(item?.kind, 'submenu');
  if (item?.kind !== 'submenu') assert.fail();
  const leaf = [...original.insertLeaves][0]!;
  assert.equal(item.children[0], leaf);
  assert.equal(leaf.segment, catalog.groups[0]!.segments[0]);
  assert.ok(Object.isFrozen(item));
  assert.ok(Object.isFrozen(item.children));
  assert.ok(Object.isFrozen(original.insertLeaves));
  assert.equal('add' in original.insertLeaves, false);
  assert.equal(original.insertLeaves.has({ ...leaf }), false);
  assert.equal(undoDepth(state), 0);
  state = state.update({ selection: EditorSelection.single(5, 1) }).state;
  const selected = prepareSavedSegmentMenu(state, 0);
  assert.equal(selected.items[1], item);
  assert.equal(selected.insertLeaves, original.insertLeaves);
  assert.equal(selected.selectedText, 'irst');
  assert.ok(selected.saveLeaf);
  assert.equal(state.selection.main.anchor, 5);
  state = state.update({
    changes: { from: 0, to: 1, insert: 'X' },
    selection: EditorSelection.single(0, 2),
  }).state;
  const edited = prepareSavedSegmentMenu(state, 0);
  assert.equal(edited.publication, original.publication);
  assert.equal(edited.items[1], item);
  assert.equal(edited.insertLeaves, original.insertLeaves);
  assert.equal([...edited.insertLeaves][0], leaf);
  assert.equal(edited.selectedText, 'Xi');
  assert.equal(readExactSource(state), 'Xirst\r\nsecond');
  assert.equal(undoDepth(state), 1);
});

test('read-only admission hides prepared insertion membership without rebuilding its catalog output', () => {
  const catalog = prepareSavedSegmentCatalog(context, [
    { id: 'a', label: 'A', text: 'a', associations: context.scopes },
  ]);
  const fixture = structureState('source', 'prompt', [savedSegments, savedSegmentHost.of(host)]);
  const permission = fixture.permission;
  let state = fixture.state.update({
    effects: publishSavedSegments.of({ host, status: 'ready', catalog }),
  }).state;
  const original = prepareSavedSegmentMenu(state, 0);
  state = state.update({
    effects: permission.reconfigure(EditorState.readOnly.of(true)),
  }).state;
  assert.equal(state.readOnly, true);
  const disabled = prepareSavedSegmentMenu(state, 0);
  assert.equal(disabled.publication, original.publication);
  assert.equal(disabled.insertLeaves.size, 0);
  assert.equal(disabled.saveLeaf, null);
  assert.deepEqual(
    disabled.items.map((item) => item.kind),
    ['disabled', 'disabled'],
  );
  state = state.update({
    effects: permission.reconfigure(EditorState.readOnly.of(false)),
  }).state;
  assert.equal(state.readOnly, false);
  const restored = prepareSavedSegmentMenu(state, 0);
  assert.equal(restored.items[1], original.items[1]);
  assert.equal(restored.insertLeaves, original.insertLeaves);
  assert.equal(readExactSource(state), 'source');
  assert.equal(undoDepth(state), 0);
});

test('catalog publication and host replacement retire old rows while foreign host publication stays rejected', () => {
  const configuration = new Compartment();
  const catalog = prepareSavedSegmentCatalog(context, [
    { id: 'a', label: 'A', text: 'a', associations: context.scopes },
  ]);
  const replacement = prepareSavedSegmentCatalog(context, [
    { id: 'b', label: 'B', text: 'b', associations: context.scopes },
  ]);
  let state = structureState('source', 'prompt', [
    savedSegments,
    configuration.of(savedSegmentHost.of(host)),
  ]).state.update({
    effects: publishSavedSegments.of({ host, status: 'ready', catalog }),
  }).state;
  const old = prepareSavedSegmentMenu(state, 0);
  state = state.update({
    effects: publishSavedSegments.of({
      host,
      status: 'ready',
      catalog: replacement,
    }),
  }).state;
  const next = prepareSavedSegmentMenu(state, 0);
  assert.equal(currentSavedSegmentMenu(state, old), false);
  assert.notEqual(next.items[1], old.items[1]);
  assert.notEqual(next.insertLeaves, old.insertLeaves);
  assert.equal(next.insertLeaves.has([...old.insertLeaves][0]!), false);
  assert.equal([...next.insertLeaves][0]!.segment.id, 'b');
  state = state.update({
    effects: publishSavedSegments.of({
      host,
      status: 'pending',
      catalog: null,
    }),
  }).state;
  assert.equal(currentSavedSegmentMenu(state, next), false);
  assert.equal(state.field(savedSegments)?.insertion, null);
  const newHost: SavedSegmentHost = { ...host };
  state = state.update({
    effects: configuration.reconfigure(savedSegmentHost.of(newHost)),
  }).state;
  assert.equal(state.field(savedSegments), null);
  state = state.update({
    effects: publishSavedSegments.of({ host, status: 'ready', catalog }),
  }).state;
  assert.equal(state.field(savedSegments), null);
  assert.equal(prepareSavedSegmentMenu(state, 0).insertLeaves.size, 0);
  state = state.update({
    effects: publishSavedSegments.of({
      host: newHost,
      status: 'ready',
      catalog,
    }),
  }).state;
  const accepted = prepareSavedSegmentMenu(state, 0);
  assert.equal(accepted.publication?.host, newHost);
  assert.notEqual(accepted.items[1], old.items[1]);
  assert.equal(accepted.insertLeaves.has([...old.insertLeaves][0]!), false);
  assert.equal(readExactSource(state), 'source');
  assert.equal(undoDepth(state), 0);
});
