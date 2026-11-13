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
/** Keep a physical reorder press pending until canonical semantics arrive without renewing retired source authority. */
import assert from 'node:assert/strict';
import test, { mock } from 'node:test';
import { EditorSelection, StateEffect, type TransactionSpec } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import {
  NativeReorderEntry,
  type ReorderEntryOutcome,
  type ReorderEntryPorts,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-entry.ts';
import { reorderState } from '../transaction/reorder-fixture.ts';
import { exactReplacement } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  initializeNativeStructure,
  semanticStructures,
} from '../../../../../../frontend/sugarprompt/editor/src/structure.ts';
import {
  richProjectionExtension,
  installProjection,
} from '../../../../../../frontend/sugarprompt/editor/src/rich-projection.ts';
import { prepareProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-preparation.ts';
import {
  loraPresentation,
  prepareLoraPresentation,
  installLoraPresentation,
} from '../../../../../../frontend/sugarprompt/editor/src/lora-presentation.ts';
import { loraMedia } from '../../../../../../frontend/sugarprompt/editor/src/lora-media-state.ts';
import { sceneErrors } from '../../../../../../frontend/sugarprompt/editor/src/scene-errors.ts';
import { nativeConditioning } from '../../../../../../frontend/sugarprompt/editor/src/conditioning-state.ts';
import { documentDiagnostics } from '../../../../../../frontend/sugarprompt/editor/src/document-diagnostics.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import {
  nativeInputSettlement,
  type NativeInputSettlement,
} from '../../../../../../frontend/sugarprompt/editor/src/input-settlement.ts';

/** Observe only external input, measurement and microtask boundaries around actual canonical states. */
function fixture(rich = false) {
  const initial = reorderState(rich ? 'a, <lora:example:0.8>, c' : 'a, b, c').state;
  let state = initial.update(exactReplacement(initial, 0, 0, 'x')).state;
  const tasks: (() => void)[] = [],
    measures: Parameters<ReorderEntryPorts['measure']>[2][] = [],
    outcomes: ReorderEntryOutcome[] = [];
  let finishes = 0,
    cancels = 0,
    composing = false,
    cause: Error | null = null;
  const input: NativeInputSettlement = {
    hasCompositionInput: () => composing,
    cancelCompositionInput() {},
    hasHeldKey: () => false,
    reserveCommand() {
      return {
        finish() {
          finishes++;
        },
        cancel() {
          cancels++;
        },
      };
    },
  };
  state = state.update({
    effects: StateEffect.appendConfig.of(nativeInputSettlement.of(input)),
  }).state;
  if (rich)
    state = state.update({
      effects: StateEffect.appendConfig.of([
        nativeConditioning,
        documentDiagnostics,
        loraPresentation,
        loraMedia,
        sceneErrors,
        richProjectionExtension,
      ]),
    }).state;
  const boundary: unknown = Object.create(EditorView.prototype);
  if (!(boundary instanceof EditorView)) throw new Error('entry-fixture-view');
  const view = boundary;
  Object.defineProperties(view, {
    state: { get: () => state },
    compositionStarted: { get: () => false },
    focus: { value: () => {} },
  });
  const scheduler = mock.method(globalThis, 'queueMicrotask', (task: () => void) =>
    tasks.push(task),
  );
  const entry = new NativeReorderEntry(view, {
    available: () => true,
    measure(_state, _pointer, settle) {
      if (cause) throw cause;
      measures.push(settle);
    },
    retireMeasurement() {},
    complete(outcome) {
      outcomes.push(outcome);
    },
    failed() {},
  });
  return {
    entry,
    tasks,
    measures,
    outcomes,
    counts: () => ({ finishes, cancels }),
    update(spec: TransactionSpec) {
      state = state.update(spec).state;
      entry.update();
    },
    publish() {
      state = initializeNativeStructure(state);
      entry.update();
    },
    /** Publish actual current rich output independently of semantic syntax readiness. */
    paint() {
      const semantic = state.field(semanticStructures).native;
      if (!semantic) throw new Error('Missing actual semantic preparation');
      state = state.update({
        effects: installProjection.of(prepareProjection(state, semantic)),
      }).state;
      entry.update();
    },
    /** Rebind the same service records through their source-current presentation owner. */
    rebind() {
      state = state.update({
        effects: installLoraPresentation.of(
          prepareLoraPresentation(state, state.field(loraPresentation).snapshot),
        ),
      }).state;
      entry.update();
    },
    compose() {
      composing = true;
      entry.update();
    },
    fail(error: Error) {
      cause = error;
    },
    task() {
      const task = tasks.shift();
      if (!task) throw new Error('missing-entry-task');
      task();
    },
    destroy() {
      entry.destroy();
      scheduler.mock.restore();
    },
  };
}

test('the original pending Alt press measures only after source-current semantic publication and settles once', () => {
  const f = fixture();
  try {
    assert.equal(f.entry.begin(), true);
    assert.equal(f.entry.pending, true);
    assert.equal(f.measures.length, 0);
    f.entry.update();
    f.entry.update();
    assert.equal(f.tasks.length, 1);
    f.task();
    assert.equal(f.tasks.length, 0);
    assert.equal(f.entry.pending, true);
    assert.equal(f.measures.length, 0);
    f.publish();
    assert.equal(f.measures.length, 0);
    f.task();
    assert.equal(f.measures.length, 1);
    f.measures[0]!(0);
    assert.equal(f.entry.pending, false);
    assert.equal(f.outcomes[0]?.status, 'accepted');
    assert.deepEqual(f.counts(), { finishes: 1, cancels: 0 });
  } finally {
    f.destroy();
  }
});
for (const retirement of [
  'release',
  'source-return',
  'selection-return',
  'context',
  'composition',
  'destroy',
] as const)
  test(`a waiting press cannot revive after ${retirement}`, () => {
    const f = fixture();
    try {
      assert.equal(f.entry.begin(), true);
      f.publish();
      if (retirement === 'release') f.entry.cancel();
      if (retirement === 'destroy') f.entry.destroy();
      if (retirement === 'composition') f.compose();
      if (retirement === 'context') f.update({ effects: invalidateCommandContext.of(null) });
      if (retirement === 'source-return') {
        f.update({ changes: { from: 0, insert: 'z' } });
        f.update({ changes: { from: 0, to: 1 } });
      }
      if (retirement === 'selection-return') {
        f.update({ selection: EditorSelection.single(2) });
        f.update({ selection: EditorSelection.single(0) });
      }
      while (f.tasks.length) f.task();
      assert.equal(f.measures.length, 0);
      assert.equal(f.entry.pending, false);
      assert.deepEqual(f.counts(), { finishes: 0, cancels: 1 });
    } finally {
      f.destroy();
    }
  });

test('measurement failure after readiness retains its cause and releases only the original reservation', () => {
  const f = fixture();
  try {
    assert.equal(f.entry.begin(), true);
    const cause = new Error('measurement failed');
    f.fail(cause);
    f.publish();
    assert.throws(
      () => f.task(),
      (error) => error === cause,
    );
    assert.equal(f.entry.pending, false);
    assert.deepEqual(f.counts(), { finishes: 0, cancels: 1 });
  } finally {
    f.destroy();
  }
});

test('pending primary entry preserves original and latest physical coordinates through semantic readiness', () => {
  const f = fixture();
  try {
    const press = { pointer: 7, x: 10, y: 20 },
      motion = { pointer: 7, x: 30, y: 40 };
    assert.equal(f.entry.begin(press), true);
    assert.equal(f.entry.ownsPointer(7), true);
    assert.equal(f.entry.motion(motion), true);
    f.publish();
    f.task();
    f.measures[0]!(0);
    const outcome = f.outcomes[0];
    assert.equal(outcome?.status, 'accepted');
    if (outcome?.status !== 'accepted') throw new Error('No canonical pointer capture');
    assert.equal(outcome.pointer, press);
    assert.equal(outcome.motion, motion);
    assert.deepEqual(f.counts(), { finishes: 1, cancels: 0 });
  } finally {
    f.destroy();
  }
});

test('resource rebinding waits for current rich paint and retires earlier measurement callbacks', () => {
  const f = fixture(true);
  try {
    assert.equal(f.entry.begin(), true);
    f.publish();
    f.task();
    assert.equal(f.measures.length, 0);
    f.paint();
    f.task();
    assert.equal(f.measures.length, 1);
    const stale = f.measures[0]!;
    f.rebind();
    stale(0);
    f.task();
    assert.equal(f.entry.pending, true);
    assert.equal(f.measures.length, 1);
    assert.equal(f.counts().finishes, 0);
    f.paint();
    f.task();
    assert.equal(f.measures.length, 2);
    stale(1);
    assert.equal(f.outcomes.length, 0);
    f.measures[1]!(0);
    assert.equal(f.outcomes[0]?.status, 'accepted');
    assert.equal(f.counts().finishes, 1);
  } finally {
    f.destroy();
  }
});
