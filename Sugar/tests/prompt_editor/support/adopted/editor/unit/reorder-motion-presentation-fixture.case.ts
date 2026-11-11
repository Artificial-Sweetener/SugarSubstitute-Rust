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
/** Native presentation fixture delivery and teardown retain finite protocol roles and raw failures. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

test('native delivery refuses a requeued callback without an actual READ', () => {
  const f = motionPresentationFixture();
  /** This hostile external callback has no native request and cannot claim another legal owner phase. */
  const requeue = (): void => {
    f.jobs.push(requeue);
  };
  try {
    f.jobs.push(requeue);
    assert.throws(() => f.deliver(), /native-frame-without-read-progress/u);
    assert.equal(f.jobs.length, 1);
  } finally {
    f.destroy();
  }
});

test('native delivery rejects a foreign array without treating it as the exact native layer', () => {
  const f = motionPresentationFixture();
  try {
    f.view.requestMeasure({ key: f.paint.layerRoot, read: () => [] });
    assert.throws(() => f.deliver(), /foreign-native-read-array/u);
  } finally {
    f.destroy();
  }
});

test('native delivery retains the raw callback exception before checking READ progress', () => {
  const f = motionPresentationFixture(),
    primary = Object.freeze({ code: 'controlled-native-frame-failure' });
  try {
    f.jobs.push(() => {
      // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
      throw primary;
    });
    assert.throws(
      () => f.deliver(),
      (cause) => cause === primary,
    );
  } finally {
    f.destroy();
  }
});

for (const secondary of [false, true])
  test(`fixture retirement attempts all native owners and preserves ordered raw causes, secondary=${secondary}`, () => {
    const originalQueue = globalThis.queueMicrotask,
      f = motionPresentationFixture(),
      primary = Object.freeze({ code: 'controlled-presentation-retirement' }),
      cleanup = Object.freeze({ code: 'controlled-widget-retirement' }),
      presentationDestroy = f.presentation.destroy.bind(f.presentation),
      widgetDestroy = f.widget.destroy.bind(f.widget),
      layerDestroy = f.paint.destroyLayer.bind(f.paint),
      calls: string[] = [];
    try {
      f.presentation.destroy = () => {
        calls.push('presentation');
        presentationDestroy();
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw primary;
      };
      f.widget.destroy = (root) => {
        calls.push('widget');
        widgetDestroy(root);
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (secondary) throw cleanup;
      };
      f.paint.destroyLayer = () => {
        calls.push('layer');
        layerDestroy();
      };
      f.jobs.push(() => {});
      assert.throws(
        () => f.destroy(),
        (cause) =>
          secondary
            ? cause instanceof AggregateError &&
              cause.errors[0] === primary &&
              cause.errors[1] === cleanup &&
              cause.errors.length === 2
            : cause === primary,
      );
      assert.deepEqual(calls, ['presentation', 'widget', 'layer']);
      assert.equal(f.paint.layerRoot.isConnected, false);
      assert.equal(f.jobs.length, 0);
      assert.equal(globalThis.queueMicrotask, originalQueue);
    } finally {
      f.presentation.destroy = presentationDestroy;
      f.widget.destroy = widgetDestroy;
      f.paint.destroyLayer = layerDestroy;
      f.destroy();
    }
  });

test('computed-style transport resolves empty inline opacity to one and retains explicit zero/fractional values', () => {
  const f = motionPaintFixture();
  try {
    for (const [inline, resolved] of [
      ['', '1'],
      ['0', '0'],
      ['0.5', '0.5'],
      ['1', '1'],
    ] as const) {
      f.scroller.style.opacity = inline;
      const computed = f.document.defaultView.getComputedStyle(f.scroller);
      assert.equal(computed.opacity, resolved);
      assert.equal(computed.getPropertyValue('opacity'), resolved);
    }
  } finally {
    f.destroyLayer();
  }
});
