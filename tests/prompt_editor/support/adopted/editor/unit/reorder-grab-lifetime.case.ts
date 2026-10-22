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
/** Actual session/measure owners preserve current press intent and native source/history through finite scheduling. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { type Transaction, type TransactionSpec } from '@codemirror/state';
import { undoDepth } from '@codemirror/commands';
import { reorderGrabCursor } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-cursor.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { pointer, key } from './reorder-session-fixture.ts';
import { paintFixture, pointerTransferFixture, drain } from './reorder-marker-fixture.ts';
/** Observe the current real fixture state after each supported cursor publication. */
function fixture() {
  const f = paintFixture();
  return {
    ...f,
    get state() {
      return f.f.state;
    },
  };
}
test('primary subthreshold press has finite grabbing ownership without proxy/source/history changes', () => {
  const f = fixture();
  try {
    const selection = f.state.selection;
    f.owner.pointerdown(pointer(1, 14, 12));
    assert.ok(f.state.field(reorderGrabCursor));
    drain(f);
    f.owner.pointermove(pointer(1, 17, 15));
    assert.ok(f.state.field(reorderGrabCursor));
    assert.equal(
      f.nodes.filter((node) => node.className === 'sp-reorder-proxy' && node.style.left !== '')
        .length,
      0,
    );
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state), 0);
    f.owner.pointerup(pointer(1, 17, 15));
    assert.equal(f.state.field(reorderGrabCursor), null);
    assert.equal(f.captures.size, 0);
  } finally {
    f.destroy();
  }
});

for (const release of ['pointerup', 'Alt'] as const)
  test(`one-time ${release} cursor publication failure retires the original complete gesture`, () => {
    const f = fixture(),
      cause = new Error('controlled-cursor-release-publication');
    try {
      const selection = f.state.selection;
      f.owner.pointerdown(pointer());
      drain(f);
      f.owner.pointermove(pointer(1, 37, 38));
      drain(f);
      const proxy = f.nodes.find((node) => node.className === 'sp-reorder-proxy')!;
      assert.ok(f.captures.size && f.state.field(reorderGrabCursor));
      const dispatch = f.view.dispatch.bind(f.view);
      let fail = true;
      /** Fail once before the real source-neutral state publication; cleanup uses the genuine remaining transaction transport. */
      f.view.dispatch = ((...inputs: readonly (Transaction | TransactionSpec)[]) => {
        if (fail) {
          fail = false;
          throw cause;
        }
        dispatch(...(inputs as Parameters<typeof f.view.dispatch>));
      }) as typeof f.view.dispatch;
      assert.throws(
        () =>
          release === 'pointerup'
            ? f.owner.pointerup(pointer(1, 37, 38))
            : f.owner.keyup(key('Alt')),
        (error) => error === cause,
      );
      drain(f);
      assert.equal(f.state.field(reorderGrabCursor), null);
      assert.equal(f.state.field(reorderPreview).value, null);
      assert.equal(f.captures.size + f.frames.size, 0);
      assert.equal(proxy.removed, true);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });

test('old cursor release failure cannot retire a reentrant newer primary press', () => {
  const transport = pointerTransferFixture(),
    f = {
      ...transport,
      get state() {
        return transport.f.state;
      },
    },
    cause = new Error('controlled-old-cursor-release');
  try {
    f.owner.pointerdown(pointer());
    drain(f);
    const old = f.state.field(reorderGrabCursor),
      dispatch = f.view.dispatch.bind(f.view);
    let reenter = true;
    /** A real newer pointer hold publishes through the same transport before the old callback reports failure. */
    f.view.dispatch = ((...inputs: readonly (Transaction | TransactionSpec)[]) => {
      if (reenter) {
        reenter = false;
        f.owner.pointerdown(f.down(0, 2, 20, 15));
        throw cause;
      }
      dispatch(...(inputs as Parameters<typeof f.view.dispatch>));
    }) as typeof f.view.dispatch;
    assert.throws(
      () => f.owner.pointerup(pointer()),
      (error) => error === cause,
    );
    drain(f);
    assert.ok(f.state.field(reorderGrabCursor));
    assert.notEqual(f.state.field(reorderGrabCursor), old);
    assert.deepEqual([...f.captures], [2]);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
    f.owner.keydown(key('Escape'));
    drain(f);
    assert.equal(f.state.field(reorderGrabCursor), null);
    assert.equal(f.captures.size + f.frames.size, 0);
  } finally {
    f.destroy();
  }
});
test('crossed threshold waits original measured geometry and final release commits once after it settles', () => {
  const f = fixture();
  try {
    f.owner.pointerdown(pointer(1, 14, 12));
    f.measure();
    f.task();
    f.owner.pointermove(pointer(1, 14, 24));
    assert.equal(f.state.field(reorderPreview).value?.order.order.length, 3);
    assert.equal(f.nodes.filter((node) => node.style.left !== '').length, 0);
    f.owner.pointerup(pointer(1, 14, 24));
    assert.ok(f.captures.size);
    drain(f);
    assert.equal(f.captures.size, 0);
    assert.equal(f.state.field(reorderGrabCursor), null);
    assert.equal(readExactSource(f.state), 'b, a, c');
    assert.equal(undoDepth(f.state), 1);
  } finally {
    f.destroy();
  }
});
for (const [x, y] of [
  [12, 14],
  [27, 26],
] as const)
  test(`noncentral press ${x}:${y} moves proxy by pointer delta and Escape cleans finite resources`, () => {
    const f = fixture();
    try {
      f.owner.pointerdown(pointer(1, y, x));
      drain(f);
      f.owner.pointermove(pointer(1, y + 17, x + 23));
      drain(f);
      const proxy = f.nodes.find((node) => node.className === 'sp-reorder-proxy');
      assert.ok(proxy);
      const surface = f.nodes.find((node) => node.className.includes('sp-reorder-drag-surface'));
      assert.ok(surface);
      assert.equal(surface.style.transform, 'translate(33px, 27px)');
      assert.equal(surface.style.top, '0px');
      assert.equal(proxy.style.width, '20px');
      assert.equal(surface.style.width, '20px');
      assert.equal(surface.style.height, '20px');
      assert.equal(surface.style.boxSizing, 'border-box');
      assert.equal(surface.style.border, '0');
      assert.equal(surface.style.padding, '0');
      assert.equal(surface.style.background, 'transparent');
      assert.equal(proxy.style.position, 'absolute');
      assert.equal(proxy.style.left, '0px');
      assert.equal(proxy.style.top, '0px');
      f.owner.keydown(key('Escape'));
      drain(f);
      assert.equal(f.state.field(reorderGrabCursor), null);
      assert.equal(f.captures.size + f.frames.size, 0);
      assert.equal(proxy.removed, true);
      assert.equal(surface.removed, true);
      assert.equal(readExactSource(f.state), 'a, b, c');
      assert.equal(undoDepth(f.state), 0);
    } finally {
      f.destroy();
    }
  });
test('geometry read failure retires cursor and capture and preserves original error', () => {
  const f = fixture(),
    cause = new Error('controlled-grab-read-failure');
  try {
    f.owner.pointerdown(pointer());
    f.measure();
    f.task();
    f.view.contentDOM.querySelectorAll = () => {
      throw cause;
    };
    assert.throws(
      () => f.measure(),
      (error) => error === cause,
    );
    while (f.tasks.length) f.task();
    assert.equal(f.state.field(reorderGrabCursor), null);
    assert.equal(f.captures.size + f.frames.size, 0);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('shadow surface preparation failure retires its partial node and every existing gesture resource', () => {
  const f = fixture(),
    cause = new Error('controlled-shadow-surface-preparation');
  try {
    f.owner.pointerdown(pointer(1, 14, 12));
    drain(f);
    const document = f.view.dom.ownerDocument,
      create = document.createElement.bind(document);
    document.createElement = ((name: string) => {
      const node = create(name),
        setProperty = node.style.setProperty.bind(node.style);
      node.style.setProperty = (property: string, value: string | null, priority?: string) => {
        if (node.className.includes('sp-reorder-drag-surface')) throw cause;
        setProperty(property, value, priority);
      };
      return node;
    }) as Document['createElement'];
    assert.throws(
      () => {
        f.owner.pointermove(pointer(1, 31, 35));
        drain(f);
      },
      (error) => error === cause,
    );
    while (f.tasks.length) f.task();
    const surface = f.nodes.find((node) => node.className.includes('sp-reorder-drag-surface'));
    assert.ok(surface);
    assert.equal(surface.removed, true);
    assert.equal(
      f.nodes.every((node) => node.removed),
      true,
    );
    assert.equal(f.state.field(reorderGrabCursor), null);
    assert.equal(f.state.field(reorderPreview).value, null);
    assert.equal(f.captures.size + f.frames.size, 0);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});

test('source-current pending entry cancellation retires grabbing before any native capture', () => {
  const f = fixture();
  try {
    f.view.posAtCoords = ((_coordinates: { x: number; y: number }, precise?: false) =>
      precise === false ? 0 : null) as typeof f.view.posAtCoords;
    f.owner.pointerdown(pointer());
    assert.ok(f.state.field(reorderGrabCursor));
    drain(f);
    assert.equal(f.state.field(reorderGrabCursor), null);
    assert.equal(f.captures.size + f.frames.size, 0);
    assert.equal(readExactSource(f.state), 'a, b, c');
    assert.equal(undoDepth(f.state), 0);
  } finally {
    f.destroy();
  }
});
