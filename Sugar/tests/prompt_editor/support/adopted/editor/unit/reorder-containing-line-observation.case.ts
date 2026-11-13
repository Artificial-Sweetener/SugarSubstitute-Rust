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
/** Real mounted geometry shares only immutable containing-line observations within one read. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import { ReorderMountedLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-layout.ts';
import { ReorderLineLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-line-layout.ts';
import type { MountedReorderTarget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-target.ts';

/** Controlled native transports expose actual owner output and independent call counts. */
function observationFixture(count: number, lineCount = 1) {
  const calls = {
    lineStyles: 0,
    lineBounds: 0,
    targetStyles: 0,
    targetBounds: 0,
    targetFragments: 0,
  };
  let width = 300,
    padding = '0px',
    lineLeft = 5,
    physicalWidth = 10;
  let observe: (frontier: string) => void = () => {};
  const lines = Array.from({ length: lineCount }, () => ({
    getBoundingClientRect() {
      calls.lineBounds++;
      observe('line-bounds');
      return { left: lineLeft, width };
    },
  }));
  const content = {};
  const view = {
    contentDOM: content,
    themeClasses: 'native-theme',
    dom: {
      ownerDocument: {
        defaultView: {
          innerWidth: 800,
          innerHeight: 600,
          devicePixelRatio: 1,
          getComputedStyle(node: unknown) {
            const line = lines.includes(node as (typeof lines)[number]);
            if (line) calls.lineStyles++;
            else if (node !== content) calls.targetStyles++;
            observe(line ? 'line-style' : node === content ? 'content' : 'target-style');
            return {
              font: '14px sans-serif',
              width: `${width}px`,
              get paddingLeft() {
                if (line) observe('line-style-field');
                return padding;
              },
              paddingRight: '0px',
              textIndent: '0px',
              direction: 'ltr',
              unicodeBidi: 'normal',
            };
          },
        },
      },
    },
  } as unknown as EditorView;
  const targets: MountedReorderTarget[] = Array.from({ length: count }, (_, index) => {
    const rect = {
      left: index * 12,
      right: index * 12 + 10,
      top: 20,
      bottom: 40,
      width: 10,
      height: 20,
    };
    const node = {
      isConnected: true,
      closest: () => lines[index % lineCount],
      getBoundingClientRect() {
        calls.targetBounds++;
        observe('target-bounds');
        return { ...rect };
      },
      getClientRects() {
        calls.targetFragments++;
        observe('target-fragments');
        return [
          {
            ...rect,
            get width() {
              observe('physical-width');
              return physicalWidth;
            },
          },
        ];
      },
    } as unknown as HTMLElement;
    return { node, kind: 'chip', index, origins: [index], leases: [] };
  });
  return {
    owner: new ReorderMountedLayout(view),
    targets,
    calls,
    changeLine() {
      width++;
      lineLeft++;
      padding = '1px';
    },
    setLineWidth(value: number) {
      width = value;
    },
    setPhysicalWidth(value: number) {
      physicalWidth = value;
    },
    onObserve(callback: (frontier: string) => void) {
      observe = callback;
    },
  };
}

test('361 actual targets share one line read while every target retains native observation and exact geometry', () => {
  const f = observationFixture(361);
  try {
    const first = f.owner.measure(f.targets)!;
    assert.equal(first.length, 361);
    assert.deepEqual(
      first,
      Array.from({ length: 361 }, (_, index) => ({
        kind: 'chip',
        index,
        left: index * 12,
        right: index * 12 + 10,
        top: 20,
        bottom: 40,
      })),
    );
    assert.deepEqual(f.calls, {
      lineStyles: 1,
      lineBounds: 1,
      targetStyles: 361,
      targetBounds: 361,
      targetFragments: 361,
    });
    assert.equal(f.owner.measure(f.targets) === first, true);
    assert.deepEqual(f.calls, {
      lineStyles: 2,
      lineBounds: 2,
      targetStyles: 722,
      targetBounds: 722,
      targetFragments: 722,
    });
    f.changeLine();
    const changed = f.owner.measure(f.targets)!;
    assert.equal(changed === first, false);
    assert.deepEqual(changed, first);
    assert.equal(f.owner.measure(f.targets) === changed, true);
    assert.equal(f.calls.lineStyles, 4);
    assert.equal(f.calls.lineBounds, 4);
  } finally {
    f.owner.destroy();
  }
});

test('distinct actual containing lines receive separate observations in every invocation', () => {
  const f = observationFixture(361, 2);
  try {
    const first = f.owner.measure(f.targets)!;
    assert.equal(first.length, 361);
    assert.equal(f.calls.lineStyles, 2);
    assert.equal(f.calls.lineBounds, 2);
    assert.equal(f.owner.measure(f.targets) === first, true);
    assert.equal(f.calls.lineStyles, 4);
    assert.equal(f.calls.lineBounds, 4);
    assert.equal(f.calls.targetStyles, 722);
    assert.equal(f.calls.targetBounds, 722);
    assert.equal(f.calls.targetFragments, 722);
  } finally {
    f.owner.destroy();
  }
});

for (const frontier of [
  'content',
  'line-style',
  'line-style-field',
  'line-bounds',
  'target-style',
  'target-bounds',
  'target-fragments',
]) {
  test(`reentrant retirement at ${frontier} cannot publish mounted geometry`, () => {
    const f = observationFixture(2);
    try {
      f.onObserve((observed) => {
        if (observed === frontier) f.owner.destroy();
      });
      assert.equal(f.owner.measure(f.targets) === null, true);
      f.onObserve(() => {});
      assert.equal(f.owner.measure(f.targets) === null, true);
    } finally {
      f.owner.destroy();
    }
  });
}

test('line and physical rectangle rejection retain finite and negative native bounds', () => {
  for (const width of [0, -1, Number.NaN, Number.POSITIVE_INFINITY]) {
    const f = observationFixture(2);
    try {
      f.setLineWidth(width);
      assert.equal(f.owner.measure(f.targets) === null, true);
    } finally {
      f.owner.destroy();
    }
  }
  for (const width of [-1, Number.NaN, Number.POSITIVE_INFINITY]) {
    const f = observationFixture(2);
    try {
      f.setPhysicalWidth(width);
      assert.equal(f.owner.measure(f.targets) === null, true);
    } finally {
      f.owner.destroy();
    }
  }
});

test('retirement during geometry assembly cannot write a borrowed line measurement', () => {
  const f = observationFixture(1),
    lines = new ReorderLineLayout(),
    lease = lines.borrow('controlled-line', () => ({
      node: { remove() {} } as unknown as HTMLElement,
      destroy() {},
    }));
  let assembling = false,
    writes = 0;
  const observedLease: typeof lease = {
    ...lease,
    geometry(context) {
      assembling = true;
      return lease.geometry(context);
    },
    measure(context, observed) {
      writes++;
      lease.measure(context, observed);
    },
  };
  try {
    f.targets[0] = { ...f.targets[0]!, leases: [observedLease] };
    f.onObserve((frontier) => {
      if (assembling && frontier === 'physical-width') f.owner.destroy();
    });
    assert.equal(f.owner.measure(f.targets) === null, true);
    assert.equal(assembling, true);
    assert.equal(writes, 0);
  } finally {
    f.owner.destroy();
    lines.destroy();
  }
});

for (const cause of [null, undefined, new Error('controlled-native-observation')]) {
  test('unexpected native getter failures preserve their raw identity', () => {
    const f = observationFixture(2);
    try {
      f.onObserve((frontier) => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (frontier === 'line-style-field') throw cause;
      });
      let caught = false;
      try {
        f.owner.measure(f.targets);
      } catch (actual) {
        caught = true;
        assert.equal(actual === cause, true);
      }
      assert.equal(caught, true);
    } finally {
      f.owner.destroy();
    }
  });
}
