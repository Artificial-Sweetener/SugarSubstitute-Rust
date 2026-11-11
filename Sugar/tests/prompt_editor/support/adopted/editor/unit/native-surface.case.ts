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
/** Controlled focusout transitions exercise the actual surface owner and native immutable history without claiming platform input proof. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorSelection, Transaction } from '@codemirror/state';
import { undo, undoDepth, redoDepth } from '@codemirror/commands';
import { captureFocusHandoff } from '../../../../../../frontend/sugarprompt/editor/src/focus-handoff.ts';
import { nativeSurfaceFixture } from './native-surface-fixture.ts';
import { NativeHostUpdates } from '../../../../../../frontend/sugarprompt/editor/src/native-host-updates.ts';
import { nativeSourceCommands } from '../../../../../../frontend/sugarprompt/editor/src/native-source-commands.ts';

test('retirement inherits newer permission/selection intent and reveals the current reverse native selection', async () => {
  const native = nativeSurfaceFixture(),
    surface = native.surface;
  try {
    await Promise.all([surface.focus(), native.flush()]);
    const previous = surface.view;
    native.focusout(() => {
      surface.retireHandoff('preserve-input');
      previous.dispatch({ selection: EditorSelection.single(3, 1) });
    });
    surface.retireInput(
      () => true,
      () => undefined,
    );
    await native.flush();
    assert.notStrictEqual(surface.view, previous);
    assert.equal(surface.view.hasFocus, true);
    assert.deepEqual(
      [surface.view.state.selection.main.anchor, surface.view.state.selection.main.head],
      [3, 1],
    );
    assert.deepEqual([undoDepth(surface.view.state), redoDepth(surface.view.state)], [1, 0]);
    assert.equal(undo(surface.view), true);
    assert.deepEqual([undoDepth(surface.view.state), redoDepth(surface.view.state)], [0, 1]);
    assert.equal(native.counts().destroyed, 1);
  } finally {
    surface.dispose();
  }
});

test('bundle focus captures restoration before publication; callback-selected external focus remains authoritative', async () => {
  const native = nativeSurfaceFixture(),
    surface = native.surface;
  try {
    const updates = new NativeHostUpdates({
      surface,
      configuration: native.host.configuration,
      permission: native.host.permission,
      createBaseline: () => {
        throw new Error('Baseline not requested');
      },
      alive: () => true,
      revision: () => 0,
      adoptBaseline: () => {
        throw new Error('Baseline not requested');
      },
      publish: () => {
        native.root.activeElement = native.external;
      },
    });
    const outcome = updates.apply({ mode: 'preserve', focus: 'editor' });
    assert.equal(outcome.status, 'applied');
    assert.equal(await outcome.focusReady, false);
    assert.strictEqual(native.root.activeElement, native.external);
    assert.equal(native.counts().focuses, 0);
    assert.equal(native.counts().measures, 0);
  } finally {
    surface.dispose();
  }
});

test('bundle focus retains its pre-dispatch destination when native publication chooses an external control', async () => {
  const native = nativeSurfaceFixture(),
    surface = native.surface;
  try {
    const dispatch = surface.view.dispatch.bind(surface.view);
    Object.assign(surface.view, {
      dispatch(input: Transaction) {
        dispatch(input);
        native.root.activeElement = native.external;
      },
    });
    const updates = new NativeHostUpdates({
      surface,
      configuration: native.host.configuration,
      permission: native.host.permission,
      createBaseline: () => {
        throw new Error('Baseline not requested');
      },
      alive: () => true,
      revision: () => 0,
      adoptBaseline: () => {
        throw new Error('Baseline not requested');
      },
      publish: () => undefined,
    });
    const outcome = updates.apply({
      mode: 'preserve',
      configuration: { placeholder: 'hint' },
      focus: 'editor',
    });
    assert.equal(outcome.status, 'applied');
    assert.equal(await outcome.focusReady, false);
    assert.strictEqual(native.root.activeElement, native.external);
    assert.equal(native.counts().focuses, 0);
  } finally {
    surface.dispose();
  }
});

test('invalid continuation does not consume a prepared source handle or replace a newer destination', () => {
  const native = nativeSurfaceFixture(),
    surface = native.surface;
  const commands = nativeSourceCommands(
    () => surface.view,
    () => undefined,
    () => undefined,
    {
      validate: (continuation) => surface.currentHandoff(continuation),
      accept: (admission, continuation) => {
        if (admission === 'source') surface.admitSource(continuation);
        else surface.retireHandoff('preserve-input');
      },
    },
  );
  try {
    const prepared = commands.api.prepareSourceEdit(0, 1, 'q');
    assert.equal(prepared.status, 'ready');
    if (prepared.status !== 'ready') throw new Error('Preparation unavailable');
    const old = surface.handoff(),
      current = surface.handoff(),
      state = surface.view.state;
    assert.equal(commands.api.applySourceEdit(prepared.command, old).status, 'rejected');
    assert.throws(
      () =>
        commands.api.applySourceEdit(prepared.command, {
          restore: async () => true,
          cancel() {},
        }),
      TypeError,
    );
    assert.strictEqual(surface.view.state, state);
    assert.equal(surface.currentHandoff(current), true);
    assert.equal(commands.api.applySourceEdit(prepared.command, current).status, 'applied');
  } finally {
    commands.dispose();
    surface.dispose();
  }
});

test('source command selection retains input destination while cancelling a delayed panel reply', async () => {
  const native = nativeSurfaceFixture(),
    surface = native.surface;
  const commands = nativeSourceCommands(
    () => surface.view,
    () => undefined,
    () => undefined,
    {
      validate: (continuation) => surface.currentHandoff(continuation),
      accept: (admission) =>
        surface.retireHandoff(admission === 'selection' ? 'preserve-input' : 'replace-input'),
    },
  );
  try {
    await Promise.all([surface.focus(), native.flush()]);
    const panel = surface.handoff(),
      input = surface.handoff('input');
    native.root.activeElement = native.body;
    commands.api.selectSource(3, 1);
    assert.equal(await panel.restore(), false);
    const ready = input.restore();
    await native.flush();
    assert.equal(await ready, true);
    assert.deepEqual(
      [surface.view.state.selection.main.anchor, surface.view.state.selection.main.head],
      [3, 1],
    );
  } finally {
    commands.dispose();
    surface.dispose();
  }
});

test('reserved source intent survives selection while ordinary panel intent retires; newer source and baseline still win', () => {
  const native = nativeSurfaceFixture(),
    surface = native.surface;
  try {
    const ordinary = surface.handoff();
    surface.retireHandoff('preserve-input');
    assert.equal(surface.currentHandoff(ordinary), false);
    const reserved = surface.handoff('reserved-source');
    surface.retireHandoff('preserve-input');
    surface.view.dispatch({ selection: EditorSelection.single(0) });
    assert.equal(surface.currentHandoff(reserved), true);
    surface.retireHandoff();
    assert.equal(surface.currentHandoff(reserved), false);
    const baseline = surface.handoff('reserved-source');
    surface.setState(surface.view.state);
    assert.equal(surface.currentHandoff(baseline), false);
  } finally {
    surface.dispose();
  }
});

test('malformed viewport restoration rejects before consuming the current one-shot intent', async () => {
  const native = nativeSurfaceFixture(),
    handoff = native.surface.handoff();
  try {
    for (const value of [null, false, 'invalid', 1])
      assert.throws(() => handoff.restore(value as 'preserve-viewport'), TypeError);
    assert.equal(native.surface.currentHandoff(handoff), true);
    assert.equal(native.counts().focuses, 0);
    const ready = handoff.restore('preserve-viewport');
    await native.flush();
    assert.equal(await ready, true);
    assert.equal(await handoff.restore(), false);
  } finally {
    native.surface.dispose();
  }
});

test('reserved picker acceptance keeps its original range after reverse selection changes and restores only its original intent', async () => {
  const native = nativeSurfaceFixture(),
    surface = native.surface;
  const commands = nativeSourceCommands(
    () => surface.view,
    () => undefined,
    () => undefined,
    {
      validate: (continuation) => surface.currentHandoff(continuation),
      accept: (admission, continuation) => {
        if (admission === 'source') surface.admitSource(continuation);
        else surface.retireHandoff('preserve-input');
      },
    },
  );
  try {
    const reserved = commands.api.reserveSourceEdit(1, 4);
    assert.equal(reserved.status, 'ready');
    if (reserved.status !== 'ready') throw new Error('Reservation unavailable');
    const handoff = surface.handoff('reserved-source');
    commands.api.selectSource(0);
    assert.equal(
      commands.api.applyReservedSourceEdit(reserved.command, 'z', handoff).status,
      'applied',
    );
    assert.equal(surface.view.state.doc.toString(), 'xzd');
    assert.equal(surface.view.state.selection.main.head, 2);
    assert.equal(undoDepth(surface.view.state), 2);
    const ready = handoff.restore();
    await native.flush();
    assert.equal(await ready, true);
    assert.equal(surface.view.hasFocus, true);
    assert.equal(await handoff.restore(), false);
  } finally {
    commands.dispose();
    surface.dispose();
  }
});

for (const mode of ['insert', 'range', 'prepared', 'reserved'] as const)
  test(`accepted ${mode} source continuation retains only its original handoff`, async () => {
    const native = nativeSurfaceFixture(),
      surface = native.surface;
    const commands = nativeSourceCommands(
      () => surface.view,
      () => undefined,
      () => undefined,
      {
        validate: (continuation) => surface.currentHandoff(continuation),
        accept: (admission, continuation) => {
          if (admission === 'source') surface.admitSource(continuation);
          else surface.retireHandoff('preserve-input');
        },
      },
    );
    try {
      const handoff = surface.handoff();
      if (mode === 'insert') assert.equal(commands.api.insertText('z', handoff), true);
      else if (mode === 'range') assert.equal(commands.api.replaceSource(0, 1, 'z', handoff), true);
      else if (mode === 'prepared') {
        const prepared = commands.api.prepareSourceEdit(0, 1, 'z');
        assert.equal(prepared.status, 'ready');
        if (prepared.status === 'ready')
          assert.equal(commands.api.applySourceEdit(prepared.command, handoff).status, 'applied');
      } else {
        const reserved = commands.api.reserveSourceEdit(0, 1);
        assert.equal(reserved.status, 'ready');
        if (reserved.status === 'ready')
          assert.equal(
            commands.api.applyReservedSourceEdit(reserved.command, 'z', handoff).status,
            'applied',
          );
      }
      const ready = handoff.restore();
      await native.flush();
      assert.equal(await ready, true);
      assert.equal(await handoff.restore(), false);
      const obsolete = surface.handoff();
      commands.api.replaceSource(0, 1, 'q');
      assert.equal(await obsolete.restore(), false);
      const before = surface.view.state;
      assert.equal(commands.api.insertText('r', obsolete), false);
      assert.strictEqual(surface.view.state, before);
    } finally {
      commands.dispose();
      surface.dispose();
    }
  });

test('composition focus restoration follows newer source-neutral intent, consumes once and releases readiness', async () => {
  const native = nativeSurfaceFixture(),
    surface = native.surface;
  try {
    await Promise.all([surface.focus(), native.flush()]);
    const handoff = captureFocusHandoff(surface.view, 'input');
    native.root.activeElement = native.body;
    surface.retireHandoff('preserve-input');
    surface.view.dispatch({ selection: EditorSelection.single(3, 1) });
    const ready = handoff.restore();
    await native.flush();
    assert.equal(await ready, true);
    assert.equal(await handoff.restore(), false);
    assert.equal(surface.view.hasFocus, true);
    assert.equal(native.counts().measures, 0);
  } finally {
    surface.dispose();
  }
});

for (const later of ['external', 'source', 'baseline', 'dispose'] as const)
  test(`retirement preserves a newer ${later} destination or lifetime`, async () => {
    const native = nativeSurfaceFixture(),
      surface = native.surface;
    try {
      await Promise.all([surface.focus(), native.flush()]);
      native.focusout(() => {
        if (later === 'external') native.root.activeElement = native.external;
        else if (later === 'source') surface.retireHandoff();
        else if (later === 'baseline') surface.setState(surface.view.state);
        else surface.dispose();
      });
      surface.retireInput(
        () => true,
        () => undefined,
      );
      await native.flush();
      assert.equal(native.counts().focuses, 1);
      assert.equal(native.counts().measures, 0);
      if (later === 'external') assert.strictEqual(native.root.activeElement, native.external);
      if (later === 'dispose') assert.throws(() => surface.view, /disposed/u);
    } finally {
      surface.dispose();
    }
  });
