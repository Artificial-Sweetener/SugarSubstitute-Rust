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
/** Actual native aggregate lifetime/source/history proof makes no DOM/input/geometry claims. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth } from '@codemirror/commands';
import { contextMenu } from '../../../../../../frontend/sugarprompt/editor/src/context-menu-state.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  captureSavedSegmentContext,
  prepareSavedSegmentCatalog,
} from '../../../../../../frontend/sugarprompt/services/src/saved-segments/preparation.ts';
import {
  publishSavedSegments,
  type SavedSegmentHost,
} from '../../../../../../frontend/sugarprompt/editor/src/saved-segment-menu.ts';
import { fieldFixture, fieldAction } from './field-actions-fixture.ts';
import type { FieldActionPreparation } from '../../../../../../frontend/sugarprompt/editor/src/field-actions-contracts.ts';
import {
  defaultPreferences,
  resolveFeatureProfile,
} from '../../../../../../frontend/sugarprompt/services/src/index.ts';
import {
  contextMenuTargetAt,
  prepareContextMenu,
} from '../../../../../../frontend/sugarprompt/editor/src/context-menu-preparation.ts';
const anchor = Object.freeze({ left: 120, right: 160, top: 30, bottom: 60 });
/** Every expectation consumes a genuine ready native contribution. */
function ready(outcome: FieldActionPreparation) {
  assert.equal(outcome.status, 'ready');
  if (outcome.status !== 'ready') assert.fail();
  return outcome.contribution;
}

test('aggregate capture preserves exact source/directional caret and literal insertion rows with one native undo', async () => {
  const f = fieldFixture();
  try {
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
    f.host.updates.apply({
      mode: 'preserve',
      configuration: { savedSegmentHost: host },
    });
    const catalog = prepareSavedSegmentCatalog(context, [
      {
        id: 'a',
        label: '<literal>',
        text: 'A\r\n😀',
        associations: context.scopes,
      },
    ]);
    f.host.transport.dispatch({
      effects: publishSavedSegments.of({ host, status: 'ready', catalog }),
    });
    const before = readExactSource(f.host.state()),
      selection = f.host.state().selection;
    const contribution = ready(await f.owner.prepareFieldActions(anchor));
    assert.equal(readExactSource(f.host.state()), before);
    assert.equal(f.host.state().selection, selection);
    assert.deepEqual(f.host.state().field(contextMenu)?.anchor, anchor);
    assert.equal(f.host.state().field(contextMenu)?.surface, 'field');
    assert.deepEqual(
      contribution.entries.map((entry) => (entry.kind === 'separator' ? entry.kind : entry.label)),
      [
        'Insert saved segment',
        'separator',
        'Save selected segment…',
        'separator',
        'Rich prompt rendering',
      ],
    );
    assert.equal(undoDepth(f.host.state()), 0);
    assert.equal(f.counts().captures, 1);
    assert.deepEqual(await fieldAction(contribution.entries, '<literal>').activate(), {
      status: 'accepted',
    });
    assert.equal(readExactSource(f.host.state()), '😀A\r\n😀t\r\nsecond');
    assert.equal(undoDepth(f.host.state()), 1);
    assert.equal(f.counts().captures, 1);
    assert.equal(f.counts().restores, 1);
    assert.equal(f.host.state().field(contextMenu), null);
  } finally {
    f.destroy();
  }
});

test('source/selection/permission retirement refuses old callbacks and read-only retains rendering only', async () => {
  const f = fieldFixture();
  try {
    const old = ready(await f.owner.prepareFieldActions(anchor));
    f.host.transport.dispatch({ selection: { anchor: 0 } });
    assert.deepEqual(await fieldAction(old.entries, 'Rich prompt rendering').activate(), {
      status: 'refused',
      reason: 'stale',
    });
    f.host.updates.apply({
      mode: 'preserve',
      configuration: { readOnly: true },
    });
    const readOnly = ready(await f.owner.prepareFieldActions(anchor));
    assert.deepEqual(
      readOnly.entries.map((entry) => (entry.kind === 'action' ? entry.label : entry.kind)),
      ['Rich prompt rendering'],
    );
    assert.equal(f.host.state().readOnly, true);
    assert.deepEqual(await fieldAction(readOnly.entries, 'Rich prompt rendering').activate(), {
      status: 'accepted',
    });
    assert.equal(f.host.state().readOnly, true);
    assert.equal(readExactSource(f.host.state()), '😀first\r\nsecond');
    assert.equal(undoDepth(f.host.state()), 0);
  } finally {
    f.destroy();
  }
});

test('supersession settles pending capture and stale release cannot close a newer contribution', async () => {
  const f = fieldFixture();
  try {
    const first = f.owner.prepareFieldActions(anchor),
      second = f.owner.prepareFieldActions(anchor);
    assert.deepEqual(await first, { status: 'cancelled' });
    const old = ready(await second),
      current = ready(await f.owner.prepareFieldActions(anchor));
    const installed = f.host.state().field(contextMenu);
    old.release();
    old.release();
    assert.equal(f.host.state().field(contextMenu), installed);
    current.release();
    current.release();
    assert.equal(f.host.state().field(contextMenu), null);
    assert.equal(f.counts().cancels, 3);
  } finally {
    f.destroy();
  }
});

test('reentrant publication preserves the newer pending invocation and its own result', async () => {
  const f = fieldFixture();
  try {
    let replacement: Promise<FieldActionPreparation> | undefined;
    f.onPublication(() => {
      f.onPublication(undefined);
      replacement = f.owner.prepareFieldActions({
        ...anchor,
        top: 80,
        bottom: 100,
      });
    });
    const first = await f.owner.prepareFieldActions(anchor);
    assert.deepEqual(first, { status: 'cancelled' });
    assert.ok(replacement);
    const current = ready(await replacement);
    assert.equal(f.host.state().field(contextMenu)?.anchor.top, 80);
    assert.deepEqual(await fieldAction(current.entries, 'Rich prompt rendering').activate(), {
      status: 'accepted',
    });
  } finally {
    f.destroy();
  }
});

test('all-attempt release preserves cleanup causes and settles pending work without retained field state', async () => {
  const f = fieldFixture(),
    focusCause = new Error('focus boundary failure'),
    spellingCause = new Error('spelling boundary failure');
  const pending = f.owner.prepareFieldActions(anchor);
  f.failFocusCancel(focusCause);
  f.failSpellingCancel(spellingCause);
  assert.throws(
    () => f.owner.close(false),
    (cause) =>
      cause instanceof AggregateError &&
      cause.errors[0] === focusCause &&
      cause.errors[1] === spellingCause,
  );
  await assert.rejects(pending, (cause) => cause === focusCause);
  assert.equal(f.host.state().field(contextMenu), null);
  f.failFocusCancel(undefined);
  f.failSpellingCancel(undefined);
  f.destroy();
  assert.equal(f.listeners.size, 0);
});

test('composition/disposal and malformed geometry settle without capture or native publication', async () => {
  const f = fieldFixture();
  assert.throws(() => f.owner.prepareFieldActions({ ...anchor, right: Number.NaN }), TypeError);
  assert.equal(f.counts().captures, 0);
  f.host.transport.composing = true;
  assert.deepEqual(await f.owner.prepareFieldActions(anchor), {
    status: 'unavailable',
    reason: 'composing',
  });
  f.host.transport.composing = false;
  const pending = f.owner.prepareFieldActions(anchor);
  f.owner.destroy();
  assert.deepEqual(await pending, { status: 'disposed' });
  assert.deepEqual(await f.owner.prepareFieldActions(anchor), {
    status: 'disposed',
  });
  assert.equal(f.owner.fieldActionsAvailable(), false);
  f.destroy();
});

test('utility activation uses the admitted callable/receiver and original handoff without foreign property rereads', async () => {
  const f = fieldFixture();
  try {
    let calls = 0;
    const utilities = {
      async scheduleLora(handoff: typeof f.focus) {
        assert.equal(this, utilities);
        assert.equal(handoff, f.focus);
        calls++;
        await handoff.restore();
      },
    };
    f.host.updates.apply({
      mode: 'preserve',
      configuration: {
        fieldActionUtilities: utilities,
        featureProfile: resolveFeatureProfile(defaultPreferences, {
          mode: 'library',
        }),
      },
    });
    const contribution = ready(await f.owner.prepareFieldActions(anchor));
    Object.defineProperty(utilities, 'scheduleLora', {
      get() {
        assert.fail('Late utility accessor must not run');
      },
    });
    assert.deepEqual(await fieldAction(contribution.entries, 'Schedule LoRA…').activate(), {
      status: 'accepted',
    });
    assert.equal(calls, 1);
    assert.equal(f.counts().captures, 1);
    assert.equal(f.counts().restores, 1);
  } finally {
    f.destroy();
  }
});

test('source reentry during close reports refused rather than delegated acceptance', async () => {
  const f = fieldFixture();
  try {
    const contribution = ready(await f.owner.prepareFieldActions(anchor));
    f.onPublication(() => {
      f.onPublication(undefined);
      f.host.transport.dispatch({ changes: { from: 0, insert: 'x' } });
    });
    assert.deepEqual(await fieldAction(contribution.entries, 'Rich prompt rendering').activate(), {
      status: 'refused',
      reason: 'stale',
    });
    assert.equal(f.host.configuration.values.richRendering, true);
    assert.equal(readExactSource(f.host.state()), 'x😀first\r\nsecond');
    assert.equal(undoDepth(f.host.state()), 1);
  } finally {
    f.destroy();
  }
});

test('foreign anchor reflection cannot let an older call supersede a newer preparation', async () => {
  const f = fieldFixture();
  try {
    let nested: Promise<FieldActionPreparation> | undefined;
    const hostile = new Proxy(anchor, {
      ownKeys(value) {
        nested = f.owner.prepareFieldActions({
          ...anchor,
          top: 70,
          bottom: 90,
        });
        return Reflect.ownKeys(value);
      },
    });
    assert.deepEqual(await f.owner.prepareFieldActions(hostile), {
      status: 'cancelled',
    });
    assert.ok(nested);
    const contribution = ready(await nested);
    assert.equal(f.counts().captures, 1);
    assert.equal(f.host.state().field(contextMenu)?.anchor.top, 70);
    contribution.release();
  } finally {
    f.destroy();
  }
});

test('same-source preparation during close retires the old invocation before domain delegation', async () => {
  const f = fieldFixture();
  try {
    const old = ready(await f.owner.prepareFieldActions(anchor));
    const source = readExactSource(f.host.state()),
      selection = f.host.state().selection,
      configuration = f.host.configuration.values;
    let replacement: Promise<FieldActionPreparation> | undefined;
    f.onPublication(() => {
      f.onPublication(undefined);
      replacement = f.owner.prepareFieldActions({
        ...anchor,
        top: 110,
        bottom: 140,
      });
    });
    assert.deepEqual(await fieldAction(old.entries, 'Rich prompt rendering').activate(), {
      status: 'refused',
      reason: 'stale',
    });
    assert.ok(replacement);
    const contribution = ready(await replacement),
      installed = f.host.state().field(contextMenu);
    assert.equal(installed?.anchor.top, 110);
    assert.equal(readExactSource(f.host.state()), source);
    assert.equal(f.host.state().selection, selection);
    assert.equal(f.host.configuration.values, configuration);
    assert.equal(f.host.configuration.values.richRendering, true);
    assert.equal(undoDepth(f.host.state()), 0);
    old.release();
    assert.equal(f.host.state().field(contextMenu), installed);
    contribution.release();
  } finally {
    f.destroy();
  }
});

test('ready empty storage preserves the local explanation and omits aggregate insertion', async () => {
  const f = fieldFixture();
  try {
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
    f.host.updates.apply({
      mode: 'preserve',
      configuration: { savedSegmentHost: host },
    });
    const catalog = prepareSavedSegmentCatalog(context, []);
    f.host.transport.dispatch({
      effects: publishSavedSegments.of({ host, status: 'ready', catalog }),
    });
    const state = f.host.state(),
      local = prepareContextMenu(
        state,
        contextMenuTargetAt(state, state.selection.main.head)!,
        'local',
      )!;
    assert.deepEqual(local.presets.items[1], {
      kind: 'disabled',
      label: 'Insert saved segment',
      title: 'No saved segments apply to this context.',
    });
    assert.equal(local.presets.insertLeaves.size, 0);
    const contribution = ready(await f.owner.prepareFieldActions(anchor));
    assert.deepEqual(
      contribution.entries.map((entry) => (entry.kind === 'separator' ? entry.kind : entry.label)),
      ['Save selected segment…', 'separator', 'Rich prompt rendering'],
    );
    assert.equal(readExactSource(f.host.state()), '😀first\r\nsecond');
    assert.equal(undoDepth(f.host.state()), 0);
    contribution.release();
  } finally {
    f.destroy();
  }
});
