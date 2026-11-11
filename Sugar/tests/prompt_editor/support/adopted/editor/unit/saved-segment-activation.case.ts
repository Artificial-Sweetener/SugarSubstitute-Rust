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
/** Genuine common menu admission and the extracted transport execute against real CM source/history, with inert external listeners/focus only. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { StateEffect } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { undo, undoDepth } from '@codemirror/commands';
import { fixture as compositionFixture } from '../transaction/host-configuration-history-fixture.ts';
import { NativeContextMenu } from '../../../../../../frontend/sugarprompt/editor/src/context-menu.ts';
import {
  contextMenu,
  installContextMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/context-menu-state.ts';
import { nativeFocusHandoff } from '../../../../../../frontend/sugarprompt/editor/src/focus-handoff.ts';
import {
  publishSavedSegments,
  type SavedSegmentHost,
} from '../../../../../../frontend/sugarprompt/editor/src/saved-segment-menu.ts';
import {
  captureSavedSegmentContext,
  prepareSavedSegmentCatalog,
} from '../../../../../../frontend/sugarprompt/services/src/saved-segments/preparation.ts';
import type { ContextMenuItem } from '../../../../../../frontend/sugarprompt/editor/src/context-menu-contracts.ts';

/** Storage publication, menu preparation and history are genuine; external focus is observed without browser claims. */
async function fixture() {
  const native = compositionFixture('a\r\nb'),
    host = native.host,
    listeners = new Set<unknown>();
  const document = {
    defaultView: {
      addEventListener(_name: string, listener: unknown) {
        listeners.add(listener);
      },
      removeEventListener(_name: string, listener: unknown) {
        listeners.delete(listener);
      },
    },
    addEventListener(_name: string, listener: unknown) {
      listeners.add(listener);
    },
    removeEventListener(_name: string, listener: unknown) {
      listeners.delete(listener);
    },
  };
  Object.assign(host.transport, { dom: { ownerDocument: document } });
  let restores = 0,
    cancels = 0,
    captures = 0;
  const handoff = {
    async restore() {
      restores++;
      return true;
    },
    cancel() {
      cancels++;
    },
  };
  host.transport.dispatch({
    effects: StateEffect.appendConfig.of(
      nativeFocusHandoff.of({
        capture() {
          captures++;
          return handoff;
        },
        current(value) {
          return value === handoff;
        },
        /** This literal activation transport admits only its original handoff. */
        refocus(value) {
          return value === handoff ? handoff.restore() : Promise.resolve(false);
        },
      }),
    ),
  });
  const context = captureSavedSegmentContext({
    mode: 'library',
    scopes: [{ kind: 'global', provider: null, key: 'global', label: 'Global' }],
  });
  const storage: SavedSegmentHost = {
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
  host.updates.apply({
    mode: 'preserve',
    configuration: { savedSegmentHost: storage, richRendering: false },
  });
  const catalog = prepareSavedSegmentCatalog(context, [
    {
      id: 'x',
      label: 'Literal',
      text: '😀X\rY\nZ\r\n',
      associations: context.scopes,
    },
  ]);
  host.transport.dispatch({
    selection: { anchor: 3, head: 2 },
    effects: publishSavedSegments.of({
      host: storage,
      status: 'ready',
      catalog,
    }),
  });
  const owner = new NativeContextMenu(host.transport as unknown as EditorView, () => {});
  const prepared = await owner.prepareFieldActions({
    left: 1,
    right: 1,
    top: 1,
    bottom: 1,
  });
  assert.equal(prepared.status, 'ready');
  const menu = host.state().field(contextMenu);
  assert.ok(menu);
  const leaf = [...menu.presets.insertLeaves][0]!;
  return {
    host,
    owner,
    focus: handoff,
    menu,
    leaf,
    get captures() {
      return captures;
    },
    get restores() {
      return restores;
    },
    get cancels() {
      return cancels;
    },
    /** Both finite owners release their actual resources. */
    destroy() {
      owner.destroy();
      native.destroy();
      assert.equal(listeners.size, 0);
    },
  };
}

test('genuine insertion survives source-neutral close and preserves exact output, direction and one undo', async () => {
  const f = await fixture(),
    selection = f.host.state().selection;
  try {
    await f.owner.activate(f.menu, f.leaf);
    assert.equal(f.host.state().field(contextMenu), null);
    assert.equal(f.host.source(), 'a\r\n😀X\rY\nZ\r\n');
    assert.equal(undoDepth(f.host.state()), 1);
    assert.equal(f.captures, 1);
    assert.equal(f.restores, 1);
    undo(f.host.transport);
    assert.equal(f.host.source(), 'a\r\nb');
    assert.equal(f.host.state().selection.eq(selection), true);
  } finally {
    f.destroy();
  }
});

for (const rejection of [
  'forged',
  'disabled',
  'closed',
  'source',
  'selection',
  'permission',
  'publication',
  'composition',
  'dispose',
] as const)
  test(`common saved-leaf admission rejects ${rejection} before activation transport`, async () => {
    const f = await fixture();
    try {
      if (rejection === 'source') f.host.transport.dispatch({ changes: { from: 0, insert: 'x' } });
      if (rejection === 'selection') f.host.transport.dispatch({ selection: { anchor: 0 } });
      if (rejection === 'permission')
        f.host.updates.apply({
          mode: 'preserve',
          configuration: { readOnly: true },
        });
      if (rejection === 'publication')
        f.host.transport.dispatch({
          effects: publishSavedSegments.of({
            host: f.menu.presets.publication!.host,
            status: 'unavailable',
            catalog: null,
          }),
        });
      if (rejection === 'composition') f.host.transport.composing = true;
      if (rejection === 'dispose') f.owner.destroy();
      if (rejection === 'closed')
        f.host.transport.dispatch({ effects: installContextMenu.of(null) });
      assert.equal(f.captures, rejection === 'permission' ? 2 : 1);
      const state = f.host.state(),
        source = f.host.source(),
        history = undoDepth(state),
        captures = f.captures,
        restores = f.restores,
        cancels = f.cancels;
      let item: ContextMenuItem = rejection === 'forged' ? { ...f.leaf } : f.leaf;
      if (rejection === 'disabled') {
        const disabled = f.menu.text.items.find((value) => value.kind === 'disabled');
        assert.ok(disabled);
        item = disabled;
      }
      await f.owner.activate(f.menu, item);
      assert.equal(f.host.state(), state);
      assert.equal(f.host.source(), source);
      assert.equal(undoDepth(f.host.state()), history);
      assert.equal(f.captures, captures);
      assert.equal(f.restores, restores);
      assert.equal(f.cancels, cancels);
      assert.equal(f.restores, 0);
    } finally {
      f.destroy();
    }
  });

for (const retirement of ['source', 'dispose'] as const)
  test(`closing publication rechecks ${retirement} before canonical insertion`, async () => {
    const f = await fixture();
    try {
      const dispatch = f.host.transport.dispatch.bind(f.host.transport);
      f.host.transport.dispatch = (value) => {
        dispatch(value);
        if (retirement === 'source') dispatch({ changes: { from: 0, insert: 'x' } });
        else f.owner.destroy();
      };
      await f.owner.activate(f.menu, f.leaf);
      assert.equal(f.host.source(), retirement === 'source' ? 'xa\r\nb' : 'a\r\nb');
      assert.equal(f.restores, 1);
    } finally {
      f.destroy();
    }
  });

test('close failure and focus retirement failure retain both exact causes', async () => {
  const f = await fixture(),
    primary = new Error('fixture-close'),
    cleanup = new Error('fixture-focus');
  try {
    f.focus.restore = async () => {
      throw cleanup;
    };
    f.host.transport.dispatch = () => {
      throw primary;
    };
    await assert.rejects(
      f.owner.activate(f.menu, f.leaf),
      (error: unknown) =>
        error instanceof AggregateError &&
        error.errors[0] === primary &&
        error.errors[1] === cleanup,
    );
    assert.equal(f.host.source(), 'a\r\nb');
    assert.equal(undoDepth(f.host.state()), 0);
  } finally {
    f.destroy();
  }
});

test('missing modal provider cancels the captured handoff after admitted save close', async () => {
  const f = await fixture();
  try {
    assert.ok(f.menu.presets.saveLeaf);
    await f.owner.activate(f.menu, f.menu.presets.saveLeaf);
    assert.equal(f.host.state().field(contextMenu), null);
    assert.equal(f.cancels, 1);
    assert.equal(f.restores, 0);
    assert.equal(f.host.source(), 'a\r\nb');
  } finally {
    f.destroy();
  }
});
