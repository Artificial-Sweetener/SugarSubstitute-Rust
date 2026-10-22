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
/** Actual host, permission, composition and surface owners distinguish queued input retirement from unrelated host adoption. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { registerHooks } from 'node:module';
import { readFileSync } from 'node:fs';
import type { EditorView } from '@codemirror/view';
import { Transaction, type TransactionSpec } from '@codemirror/state';
import type { HostUpdateOutcome } from '../../../../../../frontend/sugarprompt/editor/src/host-update.ts';

const observed = new URL(
  '../../../../../../frontend/sugarprompt/editor/src/composition-history.ts',
  import.meta.url,
).href;
const hook = registerHooks({
  /** Export-only observation keeps the actual native composition cancellation owner unchanged. */
  load(url, context, next) {
    const result = next(url, context);
    return url === observed
      ? {
          ...result,
          source:
            readFileSync(new URL(url), 'utf8') +
            '\nexport { CompositionHistory, nativeCompositionHistory };\n',
        }
      : result;
  },
});
const { nativeSurfaceFixture } = await import('../unit/native-surface-fixture.ts');
const { NativeHostUpdates } = await import(
  '../../../../../../frontend/sugarprompt/editor/src/native-host-updates.ts'
);
const { TypedNormalization } = await import(
  '../../../../../../frontend/sugarprompt/editor/src/typed-normalization.ts'
);
const { nativeInputSettlement } = await import(
  '../../../../../../frontend/sugarprompt/editor/src/input-settlement.ts'
);
const compositionModule = (await import(
  '../../../../../../frontend/sugarprompt/editor/src/composition-history.ts'
)) as unknown as {
  CompositionHistory: new (view: EditorView) => {
    start(): void;
    destroy(): void;
  };
  nativeCompositionHistory: unknown;
};

/** Native state/history and queued requestMeasure transport remain delegated to the existing real-owner fixture. */
function permissionRetirementControl() {
  const native = nativeSurfaceFixture(),
    surface = native.surface,
    view = surface.view;
  let alive = true;
  const composition = new compositionModule.CompositionHistory(view);
  const input = new TypedNormalization(view, () => undefined);
  let inputPlugin: unknown;
  Object.assign(view, {
    /** Acquire the opaque input plugin identity from its actual installed facet. */
    plugin(plugin: unknown) {
      inputPlugin = plugin;
      return input;
    },
  });
  view.state.facet(nativeInputSettlement).hasCompositionInput(view);
  assert.notEqual(inputPlugin, undefined);
  Object.assign(view, {
    composing: false,
    compositionStarted: true,
    /** This proof's closed plugin roster delegates composition and input settlement to their actual owners. */
    plugin: (plugin: unknown) =>
      plugin === compositionModule.nativeCompositionHistory
        ? composition
        : plugin === inputPlugin
          ? input
          : null,
  });
  const requests: (() => boolean)[] = [],
    retire = surface.retireInput.bind(surface);
  /** Observe issued currentness without reconstructing any admission policy. */
  surface.retireInput = (current, publish) => {
    requests.push(current);
    retire(current, publish);
  };
  const updates = new NativeHostUpdates({
    surface,
    configuration: native.host.configuration,
    permission: native.host.permission,
    createBaseline: () => native.host.state(),
    alive: () => alive,
    revision: () => 0,
    adoptBaseline: () => undefined,
    publish: () => undefined,
  });
  composition.start();
  input.compositionStart();
  return {
    native,
    surface,
    view,
    composition,
    input,
    requests,
    updates,
    /** Teardown follows the actual mounted lifecycle and releases the exported composition observer. */
    dispose() {
      alive = false;
      surface.dispose();
      composition.destroy();
      input.destroy();
    },
  };
}

test('queued read-only startup retirement survives a newer unrelated preserve configuration', async () => {
  const c = permissionRetirementControl();
  try {
    assert.deepEqual(c.updates.apply({ mode: 'preserve', configuration: { readOnly: true } }), {
      status: 'applied',
    });
    assert.equal(c.requests.length, 1);
    assert.equal(c.requests[0]!(), true);
    assert.deepEqual(
      c.updates.apply({
        mode: 'preserve',
        configuration: { placeholder: 'Later appearance' },
      }),
      { status: 'applied' },
    );
    assert.equal(c.requests[0]!(), true);
    await c.native.flush();
    assert.equal(c.native.counts().destroyed, 1);
    assert.notStrictEqual(c.surface.view, c.view);
    assert.equal(c.surface.view.state.readOnly, true);
    assert.equal(c.native.host.configuration.values.placeholder, 'Later appearance');
  } finally {
    c.dispose();
  }
});

test('committed revocation still schedules retirement after unrelated synchronous dispatch reentry', async () => {
  const c = permissionRetirementControl();
  const dispatch = c.view.dispatch.bind(c.view);
  let entered = false;
  let nested: HostUpdateOutcome | undefined;
  Object.assign(c.view, {
    /** Publish the real transaction before the independently newer host configuration reenters. */
    dispatch(input: Transaction | readonly Transaction[] | TransactionSpec) {
      if (Array.isArray(input)) dispatch(input as readonly Transaction[]);
      else if (input instanceof Transaction) dispatch(input);
      else dispatch(input as TransactionSpec);
      if (!entered && c.view.state.readOnly) {
        entered = true;
        nested = c.updates.apply({
          mode: 'preserve',
          configuration: { placeholder: 'Reentrant appearance' },
        });
      }
    },
  });
  try {
    assert.deepEqual(c.updates.apply({ mode: 'preserve', configuration: { readOnly: true } }), {
      status: 'rejected',
      reason: 'superseded',
    });
    assert.deepEqual(nested, { status: 'applied' });
    assert.equal(c.requests.length, 1);
    assert.equal(c.requests[0]!(), true);
    await c.native.flush();
    assert.equal(c.native.counts().destroyed, 1);
    assert.notStrictEqual(c.surface.view, c.view);
    assert.equal(c.surface.view.state.readOnly, true);
    assert.equal(c.native.host.configuration.values.placeholder, 'Reentrant appearance');
  } finally {
    c.dispose();
  }
});

for (const retirement of ['permission', 'baseline', 'input', 'view', 'disposal'] as const)
  test(`queued input retirement rejects superseding ${retirement} through its actual owner`, async () => {
    const c = permissionRetirementControl();
    try {
      if (retirement === 'view')
        c.surface.retireInput(
          () => true,
          () => undefined,
        );
      c.updates.apply({ mode: 'preserve', configuration: { readOnly: true } });
      const pending = c.requests.at(-1)!;
      assert.equal(pending(), true);
      if (retirement === 'permission')
        c.updates.apply({
          mode: 'preserve',
          configuration: { readOnly: false },
        });
      else if (retirement === 'baseline') {
        Object.assign(c.view, { compositionStarted: false });
        c.input.compositionEnd();
        c.updates.apply({ mode: 'baseline', source: 'abcd' });
      } else if (retirement === 'input') c.composition.start();
      else if (retirement === 'disposal') c.dispose();
      if (retirement !== 'view') assert.equal(pending(), false);
      await c.native.flush();
      if (retirement === 'view') assert.equal(pending(), false);
      assert.equal(
        c.native.counts().destroyed,
        retirement === 'input' || retirement === 'view' || retirement === 'disposal' ? 1 : 0,
      );
    } finally {
      c.dispose();
    }
  });

test.after(() => hook.deregister());
