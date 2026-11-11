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
/** Finite native preview measurement, renewal and teardown preserve source, selection and CodeMirror history. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { EditorState, StateEffect } from '@codemirror/state';
import { richRendering } from '../../../../../../frontend/sugarprompt/editor/src/presentation.ts';
import { undoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { previewFixture, adjusted } from './weight-feedback-fixture.ts';
import { showWeightPreview } from '../../../../../../frontend/sugarprompt/editor/src/weight-preview.ts';

test('typography and scale are copied entirely during read before literal preparation writes', () => {
  const f = previewFixture();
  let reading = false;
  const observed: string[] = [];
  try {
    globalThis.getComputedStyle = () => {
      const style = {};
      for (const [key, value] of Object.entries({
        fontFamily: 'Prepared Font',
        fontSize: '18px',
        color: 'rgb(1, 2, 3)',
        colorScheme: 'dark',
      }))
        Object.defineProperty(style, key, {
          get() {
            assert.equal(reading, true);
            observed.push(key);
            return value;
          },
        });
      return style as CSSStyleDeclaration;
    };
    Object.defineProperty(f.view, 'scaleY', {
      get() {
        assert.equal(reading, true);
        observed.push('scale');
        return 2;
      },
    });
    f.owner.show(f.session(), { x: 100, y: 90 });
    const job = f.jobs.shift()!;
    reading = true;
    const prepared = job.read(f.view);
    reading = false;
    job.write?.(prepared, f.view);
    assert.deepEqual(observed, ['fontFamily', 'fontSize', 'color', 'colorScheme', 'scale']);
    assert.equal(f.root.style.fontFamily, 'Prepared Font');
    assert.equal(f.root.style.fontSize, '35px');
    assert.equal(f.root.style.color, 'rgb(1, 2, 3)');
    assert.equal(f.root.style.colorScheme, 'dark');
    f.measure();
    assert.equal(f.root.style.visibility, 'visible');
  } finally {
    f.destroy();
  }
});

for (const retirement of ['clear', 'dispose'] as const)
  test(`queued publication cannot observe a retired view after ${retirement}`, async () => {
    const f = previewFixture();
    try {
      showWeightPreview(f.view, f.session(), { x: 100, y: 90 });
      if (retirement === 'clear') {
        f.owner.clear();
        f.rejectObservations();
      } else f.retire();
      await Promise.resolve();
      assert.equal(f.jobs.length + f.timers.size, 0);
      assert.equal(f.root.hidden, true);
      assert.equal(f.root.textContent, '');
    } finally {
      f.destroy();
    }
  });

test('queued publication captures the accepted point and cannot revive a cleared predecessor', async () => {
  const f = previewFixture(),
    point = { x: 100, y: 90 };
  try {
    showWeightPreview(f.view, f.session(), { x: 20, y: 30 });
    f.owner.clear();
    showWeightPreview(f.view, f.session(), point);
    point.x = 180;
    point.y = 40;
    await Promise.resolve();
    assert.equal(f.jobs.length, 1);
    f.measure();
    f.measure();
    assert.equal(f.root.style.left, '75px');
    assert.equal(f.root.style.top, '66px');
    assert.equal(f.timers.size, 1);
  } finally {
    f.destroy();
  }
});

test('accepted feedback paints after supported phases and expires once without a source transaction', () => {
  const f = previewFixture();
  try {
    const source = readExactSource(f.state),
      selection = f.state.selection,
      history = undoDepth(f.state);
    f.owner.show(f.session(), { x: 100, y: 90 });
    assert.equal(f.root.hidden, true);
    f.measure();
    assert.equal(f.root.style.visibility, 'hidden');
    f.measure();
    assert.equal(f.root.textContent, '1.10');
    assert.equal(f.root.style.visibility, 'visible');
    assert.equal(f.root.style.left, '75px');
    assert.equal(f.root.style.top, '66px');
    assert.equal(f.timers.size, 1);
    const [id, timer] = [...f.timers][0]!;
    assert.equal(timer.delay, 240);
    f.timers.delete(id);
    timer.callback();
    assert.equal(f.root.hidden, true);
    assert.equal(f.root.textContent, '');
    assert.equal(readExactSource(f.state), source);
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), history);
  } finally {
    f.destroy();
  }
  assert.equal(f.timers.size + f.listeners.size, 0);
  assert.equal(f.removed, 1);
});

test('renewal keeps the newest exact token and rejects an old expiry callback', () => {
  const f = previewFixture();
  try {
    f.owner.show(f.session(), { x: 100, y: 90 });
    f.measure();
    f.measure();
    const previous = [...f.timers.values()][0]!;
    f.publish(adjusted(f.state, { from: 13, to: 16 }));
    f.owner.show(f.session(), { x: 130, y: 90 });
    f.measure();
    f.measure();
    assert.equal(f.root.textContent, '2.05');
    assert.equal(f.timers.size, 1);
    previous.callback();
    assert.equal(f.root.textContent, '2.05');
    assert.equal(f.root.hidden, false);
    assert.equal(f.timers.size, 1);
  } finally {
    f.destroy();
  }
  assert.equal(f.timers.size + f.listeners.size, 0);
});

for (const phase of ['preparation', 'placement'] as const)
  for (const retirement of ['source', 'selection', 'dispose'] as const)
    test(`pending ${phase} cannot publish after ${retirement}`, () => {
      const f = previewFixture();
      try {
        f.owner.show(f.session(), { x: 100, y: 90 });
        if (phase === 'placement') f.measure();
        const pending = f.jobs.shift()!,
          measured = pending.read(f.view);
        if (retirement === 'dispose') f.owner.destroy();
        else
          f.publish(
            f.state.update(
              retirement === 'source'
                ? { changes: { from: 0, insert: 'z' } }
                : { selection: { anchor: 0 } },
            ).state,
          );
        const source = readExactSource(f.state),
          selection = f.state.selection,
          history = undoDepth(f.state);
        pending.write?.(measured, f.view);
        assert.equal(f.root.hidden, true);
        assert.equal(f.timers.size, 0);
        assert.equal(readExactSource(f.state), source);
        assert.equal(f.state.selection, selection);
        assert.equal(undoDepth(f.state), history);
      } finally {
        f.destroy();
      }
    });

for (const phase of ['preparation', 'placement'] as const)
  test(`unexpected ${phase} measurement preserves its cause and retires feedback`, () => {
    const f = previewFixture(),
      cause = new Error('controlled-feedback-failure');
    try {
      f.owner.show(f.session(), { x: 100, y: 90 });
      if (phase === 'placement') {
        f.measure();
        f.root.getBoundingClientRect = () => {
          throw cause;
        };
      } else
        globalThis.getComputedStyle = () => {
          throw cause;
        };
      const pending = f.jobs.shift()!;
      assert.throws(
        () => pending.read(f.view),
        (actual) => actual === cause,
      );
      assert.equal(f.root.hidden, true);
      assert.equal(f.root.textContent, '');
      assert.equal(f.timers.size, 0);
    } finally {
      f.destroy();
    }
    assert.equal(f.listeners.size, 0);
  });

for (const retirement of ['read-only', 'raw'] as const)
  test(`visible feedback releases its deadline after ${retirement} configuration`, () => {
    const f = previewFixture();
    try {
      f.owner.show(f.session(), { x: 100, y: 90 });
      f.measure();
      f.measure();
      assert.equal(f.timers.size, 1);
      f.publish(
        f.state.update({
          effects: StateEffect.appendConfig.of(
            retirement === 'read-only' ? EditorState.readOnly.of(true) : richRendering.of(false),
          ),
        }).state,
      );
      assert.equal(f.root.hidden, true);
      assert.equal(f.root.textContent, '');
      assert.equal(f.timers.size, 0);
    } finally {
      f.destroy();
    }
  });

test('unfinished native preedit refuses passive source feedback', () => {
  const f = previewFixture();
  try {
    Object.assign(f.view, { composing: true });
    f.owner.show(f.session(), { x: 100, y: 90 });
    assert.equal(f.root.hidden, true);
    assert.equal(f.jobs.length + f.timers.size, 0);
  } finally {
    f.destroy();
  }
});

test('deadline transport failure clears partially painted feedback and preserves the cause', () => {
  const f = previewFixture(),
    cause = new Error('controlled-preview-timer-failure');
  try {
    f.owner.show(f.session(), { x: 100, y: 90 });
    f.measure();
    Object.assign(f.view.dom.ownerDocument.defaultView!, {
      setTimeout: () => {
        throw cause;
      },
    });
    assert.throws(
      () => f.measure(),
      (actual) => actual === cause,
    );
    assert.equal(f.root.hidden, true);
    assert.equal(f.root.textContent, '');
    assert.equal(f.timers.size, 0);
  } finally {
    f.destroy();
  }
});

test('viewport shrink between measured phases caps the actual passive portal', () => {
  const f = previewFixture();
  try {
    f.owner.show(f.session(), { x: 100, y: 90 });
    f.measure();
    assert.equal(f.root.style.maxWidth, '188px');
    f.viewport.width = 40;
    f.measure();
    assert.equal(f.root.style.maxWidth, '28px');
    assert.equal(f.root.style.left, '16px');
    assert.equal(f.root.textContent, '1.10');
  } finally {
    f.destroy();
  }
});
