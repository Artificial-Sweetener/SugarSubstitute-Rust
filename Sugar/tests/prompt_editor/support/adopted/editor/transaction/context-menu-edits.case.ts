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
/** Genuine native edit handles prove the menu's finite leaf and metadata lifetime without browser-placement claims. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect } from '@codemirror/state';
import { undo, redo, undoDepth } from '@codemirror/commands';
import { structureState } from './document-structure-fixture.ts';
import { valueTarget, decodedEdit, acceptedEdit } from './document-value-fixture.ts';
import { ContextMenuEdits } from '../../../../../../frontend/sugarprompt/editor/src/context-menu-edits.ts';
import { prepareTextMenu } from '../../../../../../frontend/sugarprompt/editor/src/text-menu.ts';
import { prepareSavedSegmentMenu } from '../../../../../../frontend/sugarprompt/editor/src/saved-segment-menu.ts';
import type { ContextMenuItem } from '../../../../../../frontend/sugarprompt/editor/src/context-menu-contracts.ts';
import type { ContextMenu } from '../../../../../../frontend/sugarprompt/editor/src/context-menu-state.ts';
import {
  prepareNativeStructure,
  semanticStructures,
  installSemanticStructure,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  editorFreshness,
  invalidateCommandContext,
} from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { authorshipExtension } from '../../../../../../frontend/sugarprompt/editor/src/authorship.ts';
import {
  exactReplacement,
  readExactSource,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  followNativeEditContinuation,
  nativeEditContinuationTransaction,
} from '../../../../../../frontend/sugarprompt/editor/src/document-value-continuation.ts';

/** A controlled action provider supplies literal leaf identities backed by an actual canonical trigger command. */
function menuFixture() {
  const initial = structureState('(cat:1.10)\r\ntail', 'prompt', authorshipExtension);
  const state = initial.state.update({
    effects: installSemanticStructure.of({
      purpose: 'native',
      prepared: prepareNativeStructure(initial.state),
    }),
  }).state;
  const native = state.field(semanticStructures).native!;
  const context = {
    values: native.nativeValues,
    semantics: native.documentSemantics,
    enabled: true,
    composing: false,
  };
  const target = valueTarget(state, context, 0);
  const edit = acceptedEdit(state, {
    ...decodedEdit(context, target, 0, 0, 'dog, '),
    publication: { purpose: 'trigger' },
  });
  const leaf: Extract<ContextMenuItem, { kind: 'source-edit' }> = Object.freeze({
    kind: 'source-edit',
    purpose: 'trigger',
    label: 'model',
    title: 'Trigger words',
    replacement: edit,
  });
  const items = Object.freeze([leaf]);
  const menu: ContextMenu = {
    surface: 'local',
    utilities: null,
    scheduleItem: null,
    presets: prepareSavedSegmentMenu(state, 0),
    text: prepareTextMenu(state, { readable: false, writable: false }),
    kind: 'general',
    from: 0,
    to: 0,
    diagnostic: null,
    queue: null,
    queueItem: null,
    renderingItem: {
      kind: 'rich-rendering',
      label: 'Rich prompt rendering',
      checked: true,
    },
    actions: {
      kind: 'general',
      from: 0,
      to: 0,
      freshness: state.field(editorFreshness),
      pageHost: null,
      metadataHost: null,
      items,
      pageItem: null,
      schedule: null,
      editLeaves: new Set([leaf]),
    },
    items,
    anchor: { left: 1, right: 1, top: 1, bottom: 1 },
  };
  return { state, leaf, menu, edit, permission: initial.permission };
}

test('exact visible trigger follows native open and close, then preserves output, selection and one history event', () => {
  const f = menuFixture(),
    owner = new ContextMenuEdits();
  assert.equal(owner.prepare(f.state, f.menu), true);
  const measured = {
    ...f.menu,
    anchor: { left: 2, right: 2, top: 2, bottom: 2 },
  };
  assert.equal(owner.publish(measured), true);
  const metadata = StateEffect.define<null>();
  const open = f.state.update({ effects: metadata.of(null) });
  owner.follow([open]);
  assert.equal(owner.take(f.menu, f.leaf), null);
  assert.equal(owner.take(measured, { ...f.leaf }), null);
  const handle = owner.take(measured, f.leaf);
  assert.ok(handle);
  assert.equal(owner.take(measured, f.leaf), null);
  owner.clear();
  const close = open.state.update({ effects: metadata.of(null) });
  const advanced = followNativeEditContinuation(handle, close);
  assert.equal(advanced.status, 'ready');
  if (advanced.status !== 'ready') return;
  const accepted = nativeEditContinuationTransaction(close.state, advanced.value);
  assert.equal(accepted.status, 'ready');
  if (accepted.status !== 'ready') return;
  assert.equal(accepted.value.startState, close.state);
  assert.equal(accepted.value.newSelection, f.edit.transactions[0]!.newSelection);
  assert.equal(accepted.value.isUserEvent('input.exact'), true);
  let current = accepted.value.state;
  assert.equal(readExactSource(current), 'dog, (cat:1.10)\r\ntail');
  assert.equal(undoDepth(current), 1);
  /** The actual history owner supplies every undo and redo transaction. */
  const dispatch = (transaction: import('@codemirror/state').Transaction) => {
    current = transaction.state;
  };
  assert.equal(
    undo({
      get state() {
        return current;
      },
      dispatch,
    }),
    true,
  );
  assert.equal(readExactSource(current), '(cat:1.10)\r\ntail');
  assert.equal(
    redo({
      get state() {
        return current;
      },
      dispatch,
    }),
    true,
  );
  assert.equal(readExactSource(current), 'dog, (cat:1.10)\r\ntail');
});

test('pending and visible menus retain independent finite handles and reject foreign publication identities', () => {
  const f = menuFixture(),
    owner = new ContextMenuEdits();
  assert.equal(owner.prepare(f.state, f.menu), true);
  assert.equal(owner.publish({ ...f.menu, items: [...f.menu.items] }), false);
  assert.equal(owner.publish(f.menu), true);
  assert.equal(owner.prepare(f.state, f.menu), true);
  const metadata = f.state.update({
    effects: StateEffect.define<null>().of(null),
  });
  owner.follow([metadata]);
  owner.cancelPending();
  assert.equal(owner.publish(f.menu), false);
  assert.ok(owner.take(f.menu, f.leaf));
  assert.equal(owner.prepare(f.state, f.menu), true);
  assert.equal(owner.publish(f.menu), true);
  owner.retireVisible();
  assert.equal(owner.take(f.menu, f.leaf), null);
  assert.equal(owner.prepare(f.state, f.menu), true);
  owner.clear();
  assert.equal(owner.publish(f.menu), false);
});

test('actual source, selection, permission and context transactions retire measured trigger handles', () => {
  for (const reason of ['source', 'selection', 'permission', 'context', 'equal-source'] as const) {
    const f = menuFixture(),
      owner = new ContextMenuEdits();
    assert.equal(owner.prepare(f.state, f.menu), true);
    const transaction =
      reason === 'source'
        ? f.state.update({ changes: { from: 0, insert: 'x' } })
        : reason === 'selection'
          ? f.state.update({ selection: { anchor: 1 } })
          : reason === 'permission'
            ? f.state.update({
                effects: f.permission.reconfigure(EditorState.readOnly.of(true)),
              })
            : reason === 'context'
              ? f.state.update({ effects: invalidateCommandContext.of(null) })
              : f.state.update(
                  exactReplacement(
                    f.state,
                    0,
                    readExactSource(f.state).length,
                    readExactSource(f.state),
                  ),
                );
    owner.follow([transaction]);
    assert.equal(owner.publish(f.menu), true);
    assert.equal(owner.take(f.menu, f.leaf), null);
    assert.equal(
      undoDepth(transaction.state),
      reason === 'source' || reason === 'equal-source' ? 1 : 0,
    );
  }
});
