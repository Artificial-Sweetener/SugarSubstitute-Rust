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
/** Export-only observations exercise actual finite panel owners with inert listeners, immutable native state and controlled reentry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { registerHooks } from 'node:module';
import { undo, undoDepth } from '@codemirror/commands';
import { readFileSync } from 'node:fs';
import { StateEffect, Transaction, type TransactionSpec } from '@codemirror/state';
import type { EditorView } from '@codemirror/view';
import { nativeFocusHandoff } from '../../../../../../frontend/sugarprompt/editor/src/focus-handoff.ts';
import { editorFreshness } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  regionDraft,
  setRegionDraft,
} from '../../../../../../frontend/sugarprompt/editor/src/region-draft.ts';
import { semanticStructures } from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  exactWeightDraft,
  setExactWeightDraft,
} from '../../../../../../frontend/sugarprompt/editor/src/exact-weight-draft.ts';
import type { FocusHandoff } from '../../../../../../frontend/sugarprompt/editor/src/focus-handoff-contracts.ts';

const observations = new Map([
  [
    new URL('../../../../../../frontend/sugarprompt/editor/src/region-name.ts', import.meta.url)
      .href,
    'RegionNames',
  ],
  [
    new URL(
      '../../../../../../frontend/sugarprompt/editor/src/composition-history.ts',
      import.meta.url,
    ).href,
    'CompositionHistory, nativeCompositionHistory',
  ],
  [
    new URL('../../../../../../frontend/sugarprompt/editor/src/exact-weight.ts', import.meta.url)
      .href,
    'ExactWeights',
  ],
  [
    new URL(
      '../../../../../../frontend/sugarprompt/editor/src/weight-interactions.ts',
      import.meta.url,
    ).href,
    'WeightCommands, plugin as observedWeightPlugin',
  ],
]);
const hook = registerHooks({
  /** Append exports only; the production owner statements and supported CodeMirror APIs execute unchanged. */
  load(url, context, next) {
    const result = next(url, context),
      names = observations.get(url);
    return names
      ? {
          ...result,
          source: readFileSync(new URL(url), 'utf8') + `\nexport { ${names} };\n`,
        }
      : result;
  },
});
const { hostFixture } = await import('../transaction/host-document-fixture.ts');
const { NativeSurface } = await import(
  '../../../../../../frontend/sugarprompt/editor/src/native-surface.ts'
);

/** Exact test-only observation shape for the class exported in memory; no private CodeMirror field is inspected. */
interface RegionObservation {
  target: object | null;
  input: { element: { value: string }; destroy(): void } | null;
  finish(cancel: boolean, outside: boolean): void;
  destroy(): void;
}
/** The observation boundary exposes the original exact input close and acceptance callbacks. */
interface ExactObservation {
  target: object | null;
  input: { element: { value: string }; destroy(): void } | null;
  handoff: FocusHandoff | null;
  initial: string;
  key: symbol;
  finish(completion: 'keyboard', cancel: boolean): void;
  destroy(): void;
}
/** Only pending handoff identity is observed; command preparation and history stay with production owners. */
interface WeightObservation {
  pending: readonly { readonly handoff: FocusHandoff }[];
  destroy(): void;
}

/** Construct one actual surface using a fake finite native view transport rather than a hidden editor. */
function control(source = 'cat') {
  const host = hostFixture(source),
    body = {},
    internal = {},
    external = {};
  const root = { activeElement: internal as unknown };
  let focuses = 0;
  const callbacks: (() => void)[] = [];
  const view = host.transport as unknown as EditorView;
  Object.assign(host.transport, {
    root,
    dom: {
      contains: (value: unknown) => value === internal,
      ownerDocument: { body },
    },
    scrollDOM: { clientHeight: 50 },
    hasFocus: false,
    focus() {
      focuses++;
      root.activeElement = internal;
    },
    destroy() {},
    requestMeasure(request: NonNullable<Parameters<EditorView['requestMeasure']>[0]>) {
      callbacks.push(() => {
        const measured = request.read(view);
        request.write?.(measured, view);
      });
    },
  });
  const dispatch = host.transport.dispatch.bind(host.transport);
  host.transport.dispatch = (value: Transaction | TransactionSpec) =>
    dispatch(value instanceof Transaction ? value : host.state().update(value));
  const surface = new NativeSurface(
    { ownerDocument: { body } } as unknown as HTMLElement,
    host.state(),
    () => undefined,
    () => view,
  );
  host.transport.dispatch({
    effects: StateEffect.appendConfig.of(
      nativeFocusHandoff.of({
        capture: (purpose) => surface.handoff(purpose),
        current: (handoff) => surface.currentHandoff(handoff),
        refocus: (handoff) => surface.refocusHandoff(handoff),
      }),
    ),
  });
  return { host, view, surface, root, external, focuses: () => focuses };
}

test('absent-menu dismissal preserves an unrelated current handoff', async () => {
  const module = (await import(
    '../../../../../../frontend/sugarprompt/editor/src/context-menu.ts'
  )) as unknown as {
    NativeContextMenu: { prototype: { close(restore: boolean): void } };
  };
  const native = control(),
    handoff = native.surface.handoff();
  const owner = Object.create(module.NativeContextMenu.prototype) as {
    close(restore: boolean): void;
  };
  Object.assign(owner, {
    view: native.view,
    disposed: false,
    opening: null,
    generation: 0,
  });
  owner.close(false);
  assert.equal(native.surface.admitSource(handoff), true);
  assert.equal(native.focuses(), 0);
  native.surface.dispose();
});

test('region cancellation releases input before rechecking disposal and replacement draft identity', async () => {
  const module = (await import(
    '../../../../../../frontend/sugarprompt/editor/src/region-name.ts'
  )) as unknown as {
    RegionNames: new (view: EditorView) => RegionObservation;
  };
  const originalWindow = globalThis.window;
  globalThis.window = {
    addEventListener() {},
    removeEventListener() {},
  } as unknown as Window & typeof globalThis;
  try {
    for (const later of ['dispose', 'draft'] as const) {
      const native = control('[SEP|Old]\nbody'),
        owner = new module.RegionNames(native.view);
      const freshness = native.host.state().field(editorFreshness);
      const target = native.host.state().field(semanticStructures).native!.regions[0]!;
      const oldDraft = {
          freshness,
          scopeId: target.scopeId,
          ordinal: target.ordinal,
          width: 5,
        },
        newer = {
          freshness,
          scopeId: target.scopeId,
          ordinal: target.ordinal,
          width: 99,
        };
      native.host.transport.dispatch({ effects: setRegionDraft.of(oldDraft) });
      owner.target = { target, freshness };
      let released = false,
        laterDispatches = 0;
      const dispatch = native.host.transport.dispatch.bind(native.host.transport);
      native.host.transport.dispatch = (value) => {
        if (released) laterDispatches++;
        dispatch(value);
      };
      owner.input = {
        element: { value: '' },
        destroy() {
          if (later === 'dispose') owner.destroy();
          else dispatch({ effects: setRegionDraft.of(newer) });
          released = true;
        },
      };
      owner.finish(true, false);
      assert.equal(laterDispatches, 0);
      assert.strictEqual(
        native.host.state().field(regionDraft),
        later === 'draft' ? newer : oldDraft,
      );
      owner.destroy();
      native.surface.dispose();
    }
  } finally {
    globalThis.window = originalWindow;
  }
});

test('exact numeric acceptance carries the original pre-close handoff after a synchronous external choice', async () => {
  const exactModule = (await import(
    '../../../../../../frontend/sugarprompt/editor/src/exact-weight.ts'
  )) as unknown as {
    ExactWeights: new (view: EditorView) => ExactObservation;
  };
  const weightModule = (await import(
    '../../../../../../frontend/sugarprompt/editor/src/weight-interactions.ts'
  )) as unknown as {
    WeightCommands: new (view: EditorView) => WeightObservation;
    observedWeightPlugin: unknown;
  };
  const originalWindow = globalThis.window;
  globalThis.window = {
    addEventListener() {},
    removeEventListener() {},
    setTimeout: () => 1,
    clearTimeout() {},
  } as unknown as Window & typeof globalThis;
  const native = control(),
    exact = new exactModule.ExactWeights(native.view),
    commands = new weightModule.WeightCommands(native.view);
  try {
    Object.assign(native.host.transport, {
      plugin: (plugin: unknown) =>
        plugin === weightModule.observedWeightPlugin
          ? commands
          : { reserveCommand: () => ({ finish() {}, cancel() {} }) },
    });
    const freshness = native.host.state().field(editorFreshness),
      key = Symbol('control');
    exact.handoff = native.surface.handoff('input');
    native.host.transport.dispatch({
      effects: setExactWeightDraft.of({
        kind: 'emphasis',
        range: { from: 0, to: 3 },
        freshness,
        key,
        phase: 'editing',
      }),
    });
    exact.target = {
      owner: 'exact',
      kind: 'emphasis',
      range: { from: 0, to: 3 },
      freshness,
    };
    exact.initial = '1';
    exact.key = key;
    exact.input = { element: { value: '1.2' }, destroy() {} };
    const dispatch = native.host.transport.dispatch.bind(native.host.transport);
    native.host.transport.dispatch = (value) => {
      dispatch(value);
      native.root.activeElement = native.external;
      native.surface.retireHandoff();
    };
    exact.finish('keyboard', false);
    assert.equal(commands.pending.length, 0);
    assert.equal(native.host.state().field(exactWeightDraft), null);
    assert.equal(native.focuses(), 0);
  } finally {
    exact.destroy();
    commands.destroy();
    native.surface.dispose();
    globalThis.window = originalWindow;
  }
});

test('accepted region release rejects newer selection, source, permission, baseline and destruction before transaction preparation', async () => {
  const module = (await import(
    '../../../../../../frontend/sugarprompt/editor/src/region-name.ts'
  )) as unknown as {
    RegionNames: new (view: EditorView) => RegionObservation;
  };
  const originalWindow = globalThis.window;
  globalThis.window = { removeEventListener() {} } as unknown as Window & typeof globalThis;
  try {
    for (const later of ['selection', 'source', 'permission', 'baseline', 'destroy'] as const) {
      const host = hostFixture('[SEP|Old]\nbody'),
        view = host.transport as unknown as EditorView;
      const compositionModule = (await import(
        '../../../../../../frontend/sugarprompt/editor/src/composition-history.ts'
      )) as unknown as {
        CompositionHistory: new (view: EditorView) => { destroy(): void };
        nativeCompositionHistory: unknown;
      };
      const composition = new compositionModule.CompositionHistory(view);
      Object.assign(host.transport, {
        root: { activeElement: null },
        dom: { ownerDocument: { body: {} } },
        plugin: (plugin: unknown) =>
          plugin === compositionModule.nativeCompositionHistory ? composition : null,
      });
      const owner = new module.RegionNames(view);
      const freshness = host.state().field(editorFreshness);
      const target = host.state().field(semanticStructures).native!.regions[0]!;
      const draft = {
        freshness,
        scopeId: target.scopeId,
        ordinal: target.ordinal,
        width: 20,
      };
      host.transport.dispatch({ effects: setRegionDraft.of(draft) });
      owner.target = { target, freshness };
      let released = false,
        publications = 0;
      const dispatch = host.transport.dispatch.bind(host.transport);
      host.transport.dispatch = (value) => {
        if (released) {
          publications++;
          const cleanup = value instanceof Transaction ? value : host.state().update(value);
          assert.equal(cleanup.docChanged, false);
          assert.equal(cleanup.selection, undefined);
          assert.equal(cleanup.annotation(Transaction.addToHistory), false);
          assert.equal(cleanup.effects.length, 1);
          assert.equal(cleanup.effects[0]!.is(setRegionDraft), true);
        }
        const transactions = Array.isArray(value) ? value : [value];
        for (const transaction of transactions) {
          if (transaction instanceof Transaction)
            assert.equal(transaction.startState === host.state(), true, 'Stale native transaction');
          dispatch(transaction as Transaction | TransactionSpec);
        }
      };
      owner.input = {
        element: { value: 'New' },
        destroy() {
          if (later === 'selection') dispatch({ selection: { anchor: 5, head: 1 } });
          if (later === 'source') host.updates.apply({ mode: 'exact-edit', source: 'later' });
          if (later === 'permission')
            host.updates.apply({
              mode: 'preserve',
              configuration: { readOnly: true },
            });
          if (later === 'baseline') host.updates.apply({ mode: 'baseline', source: 'baseline' });
          if (later === 'destroy') owner.destroy();
          released = true;
        },
      };
      owner.finish(false, false);
      assert.equal(publications, later === 'selection' ? 1 : 0);
      if (later === 'selection') assert.equal(host.state().field(regionDraft), null);
      assert.equal(
        host.source() ===
          (later === 'source' ? 'later' : later === 'baseline' ? 'baseline' : '[SEP|Old]\nbody'),
        true,
      );
      if (later === 'selection')
        assert.deepEqual(
          [host.state().selection.main.anchor, host.state().selection.main.head],
          [5, 1],
        );
      owner.destroy();
      composition.destroy();
    }
  } finally {
    globalThis.window = originalWindow;
  }
});

test('accepted region without a newer release choice keeps its source edit, native history and draft cleanup', async () => {
  const module = (await import(
    '../../../../../../frontend/sugarprompt/editor/src/region-name.ts'
  )) as unknown as {
    RegionNames: new (view: EditorView) => RegionObservation;
  };
  const host = hostFixture('[SEP|Old]\nbody'),
    view = host.transport as unknown as EditorView;
  const owner = new module.RegionNames(view);
  const freshness = host.state().field(editorFreshness),
    target = host.state().field(semanticStructures).native!.regions[0]!;
  host.transport.dispatch({
    effects: setRegionDraft.of({
      freshness,
      scopeId: target.scopeId,
      ordinal: target.ordinal,
      width: 20,
    }),
  });
  owner.target = { target, freshness };
  owner.input = { element: { value: 'New' }, destroy() {} };
  const dispatch = host.transport.dispatch.bind(host.transport);
  host.transport.dispatch = (value) => {
    for (const transaction of Array.isArray(value) ? value : [value])
      dispatch(transaction as Transaction | TransactionSpec);
  };
  const originalWindow = globalThis.window;
  globalThis.window = { removeEventListener() {} } as unknown as Window & typeof globalThis;
  try {
    owner.finish(false, false);
    assert.equal(host.source() === '[SEP|New]\nbody', true);
    assert.equal(host.state().field(regionDraft), null);
    assert.equal(undoDepth(host.state()), 1);
    assert.equal(undo(host.transport), true);
    assert.equal(host.source() === '[SEP|Old]\nbody', true);
  } finally {
    owner.destroy();
    globalThis.window = originalWindow;
  }
});

test('accepted region publication rechecks newer reverse selection and teardown before further restoration', async () => {
  const module = (await import(
    '../../../../../../frontend/sugarprompt/editor/src/region-name.ts'
  )) as unknown as {
    RegionNames: new (view: EditorView) => RegionObservation;
  };
  const originalWindow = globalThis.window;
  globalThis.window = { removeEventListener() {} } as unknown as Window & typeof globalThis;
  try {
    for (const later of ['selection', 'destroy'] as const) {
      const host = hostFixture('[SEP|Old]\nbody');
      const view = host.transport as unknown as EditorView;
      const owner = new module.RegionNames(view);
      const freshness = host.state().field(editorFreshness);
      const target = host.state().field(semanticStructures).native!.regions[0]!;
      host.transport.dispatch({
        effects: setRegionDraft.of({
          freshness,
          scopeId: target.scopeId,
          ordinal: target.ordinal,
          width: 20,
        }),
      });
      owner.target = { target, freshness };
      owner.input = { element: { value: 'New' }, destroy() {} };
      let changed = false,
        subsequent = 0;
      const dispatch = host.transport.dispatch.bind(host.transport);
      host.transport.dispatch = (value) => {
        if (changed) subsequent++;
        for (const transaction of Array.isArray(value) ? value : [value])
          dispatch(transaction as Transaction | TransactionSpec);
        if (host.source() === '[SEP|New]\nbody') {
          if (later === 'selection') dispatch({ selection: { anchor: 5, head: 1 } });
          else owner.destroy();
          changed = true;
        }
      };
      owner.finish(false, false);
      assert.equal(changed, true);
      assert.equal(subsequent, 0);
      assert.equal(host.source() === '[SEP|New]\nbody', true);
      if (later === 'selection')
        assert.deepEqual(
          [host.state().selection.main.anchor, host.state().selection.main.head],
          [5, 1],
        );
      owner.destroy();
    }
  } finally {
    globalThis.window = originalWindow;
  }
});

test.after(() => hook.deregister());
