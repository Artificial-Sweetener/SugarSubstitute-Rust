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
/** Actual native menu admission delegates mode changes to atomic host adoption without competing source/history state. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { StateEffect, Transaction } from '@codemirror/state';
import {
  contextMenu,
  currentContextMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/context-menu-state.ts';
import { NativeContextMenu } from '../../../../../../frontend/sugarprompt/editor/src/context-menu.ts';
import { hostFixture } from '../transaction/host-document-fixture.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { nativeFocusHandoff } from '../../../../../../frontend/sugarprompt/editor/src/focus-handoff.ts';
import type { FocusHandoff } from '../../../../../../frontend/sugarprompt/editor/src/focus-handoff-contracts.ts';

/** Supply only the platform instanceof boundary for the targetless keyboard gesture. */
class MenuElement {}

/** Actual host/menu owners share finite external listener, measurement and focus-capability transports; browser focus is not certified. */
function fixture(readOnly = false) {
  const host = hostFixture('🙂\r\n(cat:1.200), tail', readOnly);
  const listeners = new Set<unknown>();
  const document = {
    defaultView: {
      addEventListener(_type: string, listener: unknown) {
        listeners.add(listener);
      },
      removeEventListener(_type: string, listener: unknown) {
        listeners.delete(listener);
      },
    },
    addEventListener(_type: string, listener: unknown) {
      listeners.add(listener);
    },
    removeEventListener(_type: string, listener: unknown) {
      listeners.delete(listener);
    },
  };
  const measurements: (() => void)[] = [];
  let active: FocusHandoff | null = null,
    alive = true,
    captures = 0,
    restores = 0;
  host.transport.dispatch({
    effects: StateEffect.appendConfig.of(
      nativeFocusHandoff.of({
        /** Issue one exact external capability before the actual invocation begins. */
        capture(purpose) {
          assert.equal(purpose, 'reserved-source');
          captures++;
          const handoff: FocusHandoff = {
            /** Consume only the current original capability without changing source or selection. */
            restore() {
              const current = alive && active === handoff;
              if (current) {
                active = null;
                restores++;
              }
              return Promise.resolve(current);
            },
            /** Retirement cannot consume a newer invocation's external capability. */
            cancel() {
              if (active === handoff) active = null;
            },
          };
          active = handoff;
          return handoff;
        },
        /** Expose exact external capability freshness to the genuine menu owner. */
        current: (handoff) => alive && active === handoff,
        /** This closed keyboard specimen never requests pointer refocus. */
        refocus() {
          throw new Error('unexpected-menu-pointer-refocus');
        },
      }),
    ),
    annotations: Transaction.addToHistory.of(false),
  });
  Object.assign(host.transport, {
    dom: { ownerDocument: document },
    coordsAtPos: () => ({ left: 1, right: 1, top: 1, bottom: 1 }),
    /** Queue the supported native measurement for explicit READ/WRITE delivery. */
    requestMeasure(request: NonNullable<Parameters<EditorView['requestMeasure']>[0]>) {
      measurements.push(() => {
        const view = host.transport as unknown as EditorView;
        const value = request.read(view);
        request.write?.(value, view);
      });
    },
  });
  const intents: boolean[] = [];
  const owner = new NativeContextMenu(host.transport as unknown as EditorView, (enabled) => {
    const outcome = host.updates.apply({
      mode: 'preserve',
      configuration: { richRendering: enabled },
    });
    if (outcome.status === 'applied') intents.push(enabled);
  });
  return {
    host,
    owner,
    intents,
    listeners,
    /** Public keyboard opening owns preparation, measured anchoring and genuine publication. */
    async open() {
      const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'Element');
      const previousCaptures = captures;
      try {
        Object.defineProperty(globalThis, 'Element', {
          configurable: true,
          value: MenuElement,
        });
        assert.equal(
          owner.key({
            key: 'ContextMenu',
            target: null,
            preventDefault() {},
          } as unknown as KeyboardEvent),
          true,
        );
      } finally {
        if (descriptor) Object.defineProperty(globalThis, 'Element', descriptor);
        else Reflect.deleteProperty(globalThis, 'Element');
      }
      assert.equal(captures, previousCaptures + 1);
      await Promise.resolve();
      assert.equal(measurements.length, 1);
      measurements.shift()!();
      await Promise.resolve();
      const menu = host.state().field(contextMenu);
      assert.ok(menu);
      assert.equal(currentContextMenu(host.state(), menu), true);
      return menu;
    },
    /** Observe issued and consumed external capabilities without replacing menu admission. */
    focusCounts: () => ({ captures, restores }),
    /** Attempt every owner retirement even if an earlier cleanup fails. */
    destroy() {
      const causes: unknown[] = [];
      for (const release of [
        () => owner.destroy(),
        () => {
          alive = false;
          active = null;
          measurements.length = 0;
        },
        () => host.destroy(),
        () => assert.equal(listeners.size, 0),
      ]) {
        try {
          release();
        } catch (cause) {
          causes.push(cause);
        }
      }
      if (causes.length === 1) throw causes[0];
      if (causes.length) throw new AggregateError(causes, 'rich-menu-fixture-retirement-failed');
    },
  };
}

for (const readOnly of [false, true])
  test(`checkbox switches both modes with exact source/caret/history retained, readOnly=${readOnly}`, async () => {
    const f = fixture(readOnly);
    try {
      if (!readOnly)
        f.host.transport.dispatch({
          changes: { from: 0, insert: 'prefix ' },
          userEvent: 'input.type',
        });
      f.host.transport.dispatch({ selection: { anchor: 12, head: 7 } });
      const before = f.host.state(),
        source = f.host.source();
      for (const enabled of [false, true]) {
        const menu = await f.open();
        assert.equal(menu.renderingItem.checked, !enabled);
        await f.owner.activate(menu, menu.renderingItem);
        assert.equal(f.host.state().facet(richRendering), enabled);
        assert.equal(f.host.configuration.values.richRendering, enabled);
        assert.equal(f.host.source(), source);
        assert.ok(f.host.state().selection.eq(before.selection));
        assert.equal(undoDepth(f.host.state()), undoDepth(before));
        assert.equal(redoDepth(f.host.state()), redoDepth(before));
        assert.equal(f.host.state().field(contextMenu), null);
        assert.equal(currentContextMenu(f.host.state(), menu), false);
      }
      assert.deepEqual(f.intents, [false, true]);
      assert.deepEqual(f.focusCounts(), { captures: 2, restores: 2 });
    } finally {
      f.destroy();
    }
  });

for (const rejection of [
  'forged',
  'source',
  'selection',
  'mode',
  'permission',
  'composition',
  'dispose',
] as const)
  test(`captured checkbox cannot publish after ${rejection}`, async () => {
    const f = fixture();
    try {
      const menu = await f.open();
      if (rejection === 'source') f.host.transport.dispatch({ changes: { from: 0, insert: 'x' } });
      if (rejection === 'selection') f.host.transport.dispatch({ selection: { anchor: 1 } });
      if (rejection === 'mode')
        f.host.updates.apply({
          mode: 'preserve',
          configuration: { richRendering: false },
        });
      if (rejection === 'permission')
        f.host.updates.apply({
          mode: 'baseline',
          source: f.host.source(),
          configuration: { readOnly: true },
        });
      if (rejection === 'composition') f.host.transport.composing = true;
      if (rejection === 'dispose') f.owner.destroy();
      const state = f.host.state();
      await f.owner.activate(
        menu,
        rejection === 'forged' ? { ...menu.renderingItem } : menu.renderingItem,
      );
      assert.equal(f.host.state(), state);
      assert.deepEqual(f.intents, []);
    } finally {
      f.destroy();
    }
  });

for (const replacement of ['source', 'dispose'] as const)
  test(`closing publication rechecks ${replacement} retirement before native configuration intent`, async () => {
    const f = fixture();
    try {
      const menu = await f.open(),
        dispatch = f.host.transport.dispatch.bind(f.host.transport);
      f.host.transport.dispatch = (input) => {
        dispatch(input);
        if (replacement === 'dispose') f.owner.destroy();
        else dispatch({ changes: { from: 0, insert: 'x' } });
      };
      await f.owner.activate(menu, menu.renderingItem);
      assert.deepEqual(f.intents, []);
      assert.equal(f.host.state().facet(richRendering), true);
    } finally {
      f.destroy();
    }
  });
