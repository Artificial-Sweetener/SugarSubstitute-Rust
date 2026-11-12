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
/** Literal physical-input characterization separates generic layout notifications from actual painted preview retirement. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect } from '@codemirror/state';
import type { ViewUpdate } from '@codemirror/view';
import { undoDepth } from '@codemirror/commands';
import { previewFixture } from './weight-feedback-fixture.ts';
import { invalidateCommandContext } from '../../../../../../frontend/sugarprompt/editor/src/freshness.ts';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** Reuse the real accepted-state fixture; literal mutable computed typography remains an external unit transport, not browser evidence. */
function physicalFixture() {
  const fixture = previewFixture();
  const typography = {
    fontFamily: 'Segoe UI',
    fontSize: '14px',
    color: 'rgb(0, 0, 0)',
    colorScheme: 'light',
  };
  globalThis.getComputedStyle = () => typography as CSSStyleDeclaration;
  return Object.assign(fixture, { typography });
}
/** Deliver a native notification independently from the actual measured physical inputs. */
function geometry(
  fixture: ReturnType<typeof physicalFixture>,
  flag: 'geometry' | 'viewport' | 'both' = 'geometry',
): void {
  fixture.owner.update({
    startState: fixture.state,
    state: fixture.state,
    selectionSet: false,
    geometryChanged: flag !== 'viewport',
    viewportChanged: flag !== 'geometry',
  } as ViewUpdate);
}
/** The existing two supported phases publish one canonical accepted value and finite deadline. */
function paint(fixture: ReturnType<typeof physicalFixture>): void {
  fixture.owner.show(fixture.session(), { x: 100, y: 90 });
  fixture.measure();
  fixture.measure();
  assert.equal(fixture.root.textContent, '1.10');
  assert.equal(fixture.root.hidden, false);
  assert.equal(fixture.root.style.visibility, 'visible');
  assert.equal(fixture.timers.size, 1);
}
/** Existing expiry owns actual cleanup; geometry validation cannot replace its timer object or extend its deadline. */
function expire(fixture: ReturnType<typeof physicalFixture>): void {
  const [id, timer] = [...fixture.timers][0]!;
  assert.equal(timer.delay, 240);
  fixture.timers.delete(id);
  timer.callback();
  assert.equal(fixture.root.hidden, true);
  assert.equal(fixture.root.textContent, '');
  assert.equal(fixture.timers.size, 0);
}

for (const flag of ['geometry', 'viewport', 'both'] as const)
  test(`painted preview survives equal physical inputs after ${flag} notification without renewing its original expiry`, () => {
    const f = physicalFixture();
    try {
      paint(f);
      const [id, timer] = [...f.timers][0]!,
        portal = { ...f.root.style },
        source = readExactSource(f.state),
        selection = f.state.selection,
        history = undoDepth(f.state);
      geometry(f, flag);
      geometry(f, flag);
      assert.equal(f.jobs.length, 1);
      assert.equal(f.root.hidden, false);
      f.measure();
      assert.equal(f.jobs.length, 0);
      assert.equal(f.timers.get(id), timer);
      assert.equal(f.timers.size, 1);
      assert.deepEqual(f.root.style, portal);
      assert.equal(readExactSource(f.state), source);
      assert.ok(f.state.selection.eq(selection, true));
      assert.equal(undoDepth(f.state), history);
      expire(f);
    } finally {
      f.destroy();
    }
    assert.equal(f.timers.size + f.listeners.size, 0);
    assert.equal(f.removed, 1);
  });

for (const changed of [
  'left',
  'top',
  'width',
  'height',
  'fontFamily',
  'fontSize',
  'color',
  'colorScheme',
  'scale',
] as const)
  test(`painted preview retires only after measuring an independently changed ${changed}`, () => {
    const f = physicalFixture();
    try {
      paint(f);
      if (changed === 'scale') Object.assign(f.view, { scaleY: 2 });
      else if (changed === 'fontFamily') f.typography.fontFamily = 'Changed Font';
      else if (changed === 'fontSize') f.typography.fontSize = '18px';
      else if (changed === 'colorScheme') f.typography.colorScheme = 'dark';
      else if (changed === 'color') f.typography.color = 'rgb(1, 2, 3)';
      else f.viewport[changed] += 5;
      geometry(f);
      assert.equal(f.root.hidden, false);
      assert.equal(f.timers.size, 1);
      assert.equal(f.jobs.length, 1);
      f.measure();
      assert.equal(f.root.hidden, true);
      assert.equal(f.root.textContent, '');
      assert.equal(f.timers.size, 0);
    } finally {
      f.destroy();
    }
    assert.equal(f.timers.size + f.listeners.size, 0);
  });

for (const phase of ['read', 'write'] as const)
  for (const retirement of [
    'source',
    'selection',
    'identical-selection',
    'context',
    'read-only',
    'raw',
    'clear',
    'dispose',
  ] as const)
    test(`pending physical validation ${phase} remains inert after ${retirement}`, () => {
      const f = physicalFixture();
      try {
        paint(f);
        geometry(f);
        assert.equal(f.jobs.length, 1);
        const pending = f.jobs.shift()!,
          measured = phase === 'write' ? pending.read(f.view) : null;
        if (retirement === 'dispose') f.retire();
        else if (retirement === 'clear') {
          f.owner.clear();
          f.rejectObservations();
        } else
          f.publish(
            f.state.update(
              retirement === 'source'
                ? { changes: { from: 0, insert: 'z' } }
                : retirement === 'selection'
                  ? { selection: { anchor: 0 } }
                  : retirement === 'identical-selection'
                    ? { selection: f.state.selection }
                    : {
                        effects:
                          retirement === 'context'
                            ? invalidateCommandContext.of(null)
                            : StateEffect.appendConfig.of(
                                retirement === 'read-only'
                                  ? EditorState.readOnly.of(true)
                                  : richRendering.of(false),
                              ),
                      },
            ).state,
          );
        assert.equal(f.root.hidden, true);
        assert.equal(f.timers.size, 0);
        if (phase === 'read') assert.equal(pending.read(f.view), null);
        pending.write?.(measured, f.view);
        assert.equal(f.root.hidden, true);
        assert.equal(f.root.textContent, '');
        assert.equal(f.jobs.length + f.timers.size, 0);
      } finally {
        f.destroy();
      }
    });

test('old validation cannot retire a newer painted intent or reset its pending validation', () => {
  const f = physicalFixture();
  try {
    paint(f);
    geometry(f);
    const old = f.jobs.shift()!,
      oldInputs = old.read(f.view);
    f.owner.show(f.session(), { x: 130, y: 90 });
    f.measure();
    f.measure();
    const [id, timer] = [...f.timers][0]!;
    geometry(f);
    assert.equal(f.jobs.length, 1);
    old.write?.(oldInputs, f.view);
    assert.equal(old.read(f.view), null);
    assert.equal(f.root.style.left, '105px');
    assert.equal(f.root.hidden, false);
    f.measure();
    assert.equal(f.timers.get(id), timer);
    assert.equal(f.jobs.length, 0);
    expire(f);
  } finally {
    f.destroy();
  }
});

test('expiry between physical validation phases prevents any later portal publication', () => {
  const f = physicalFixture();
  try {
    paint(f);
    geometry(f);
    const pending = f.jobs.shift()!,
      measured = pending.read(f.view);
    expire(f);
    f.rejectObservations();
    assert.equal(pending.read(f.view), null);
    pending.write?.(measured, f.view);
    assert.equal(f.root.hidden, true);
    assert.equal(f.jobs.length + f.timers.size, 0);
  } finally {
    f.destroy();
  }
});

for (const failing of ['schedule', 'read'] as const)
  test(`unexpected physical validation ${failing} failure retires resources and preserves the original cause`, () => {
    const f = physicalFixture(),
      cause = new Error('controlled-physical-validation-failure');
    try {
      paint(f);
      if (failing === 'schedule') {
        f.view.requestMeasure = () => {
          throw cause;
        };
        assert.throws(
          () => geometry(f),
          (actual) => actual === cause,
        );
      } else {
        geometry(f);
        const pending = f.jobs.shift()!;
        globalThis.getComputedStyle = () => {
          throw cause;
        };
        assert.throws(
          () => pending.read(f.view),
          (actual) => actual === cause,
        );
        f.rejectObservations();
        assert.equal(pending.read(f.view), null);
        pending.write?.(null, f.view);
      }
      assert.equal(f.root.hidden, true);
      assert.equal(f.root.textContent, '');
      assert.equal(f.timers.size, 0);
    } finally {
      f.destroy();
    }
    assert.equal(f.timers.size + f.listeners.size, 0);
  });
