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
/** Literal supported-view hits prove pointer intent delegation; native feedback/mouse cases own physical edge geometry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { heldTokenFixture, heldTokenPointer } from './held-token-fixture.ts';
import { heldProjection } from '../../../../../../frontend/sugarprompt/editor/src/projection-exposure.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';

/** Only Element identity/closest is modeled; CodeMirror hit results are independently supplied literal positions. */
class ChipTarget {
  /** Match the exact production semantic target lookup without providing browser layout. */
  closest(selector: string): this {
    assert.equal(selector, '.sp-lora-chip');
    return this;
  }
}

/** Own the temporary Element boundary and real state/plugin fixture with primary-first all-attempt retirement. */
function chipFixture(body: (value: ReturnType<typeof acquireChipFixture>) => void): void {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, 'Element');
  let fixture: ReturnType<typeof acquireChipFixture> | null = null;
  const causes: unknown[] = [];
  try {
    Object.defineProperty(globalThis, 'Element', {
      configurable: true,
      value: ChipTarget,
    });
    fixture = acquireChipFixture();
    body(fixture);
  } catch (cause) {
    causes.push(cause);
  }
  for (const retire of [
    () => fixture?.native.destroy(),
    () => {
      if (descriptor) Object.defineProperty(globalThis, 'Element', descriptor);
      else Reflect.deleteProperty(globalThis, 'Element');
    },
  ])
    try {
      retire();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'MOUSE_CHIP_PROOF_RETIREMENT');
}

/** Adapt only public view hit methods; native state, projection, gestures and history remain the actual owners. */
function acquireChipFixture() {
  const native = heldTokenFixture(undefined, 2),
    target = new ChipTarget();
  const coordinates: number[][] = [];
  let domReads = 0;
  try {
    const hit = native.view.posAtCoords.bind(native.view);
    Object.defineProperty(native.view, 'posAtCoords', {
      value: (point: { x: number; y: number }) => {
        coordinates.push([point.x, point.y]);
        return hit(point);
      },
    });
    Object.defineProperty(native.view, 'posAtDOM', {
      value: (node: unknown, offset: number) => {
        assert.equal(node, target);
        assert.equal(offset, 0);
        domReads++;
        return 6;
      },
    });
    return {
      native,
      coordinates,
      get domReads() {
        return domReads;
      },
      /** Preserve original event identity so native focus cannot remap the initial hit on its later get callback. */
      press(position: number, detail = 1, shiftKey = false) {
        return native.press({
          // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
          ...heldTokenPointer(position, detail, shiftKey),
          target,
        } as unknown as MouseEvent);
      },
    };
  } catch (primary) {
    try {
      native.destroy();
    } catch (cause) {
      throw new AggregateError([primary, cause], 'MOUSE_CHIP_PROOF_ACQUISITION');
    }
    throw primary;
  }
}

for (const edge of [6, 19])
  test(`ordinary chip press captures native edge ${edge} once before focus and retains exact source/history`, () => {
    chipFixture((f) => {
      const { native, coordinates, press } = f;
      const { event, style } = press(edge);
      native.duringRead(() => assert.fail('initial get cannot reread geometry after native focus'));
      assert.deepEqual(
        style.get(event, false, false).ranges.map((r) => [r.anchor, r.head]),
        [[edge, edge]],
      );
      native.select(style, event);
      assert.deepEqual(
        [native.state.selection.main.anchor, native.state.selection.main.head],
        [edge, edge],
      );
      assert.deepEqual(coordinates, [[edge, 0]]);
      assert.equal(f.domReads, 0);
      assert.equal(readExactSource(native.state), 'alpha <lora:real:1>, tail');
      assert.deepEqual([undoDepth(native.state), redoDepth(native.state)], [0, 0]);
    });
  });

test('single chip hits retain native Shift/multiple anchors and drag continuation', () => {
  chipFixture(({ press, coordinates }) => {
    const { event, style } = press(19, 1, true);
    assert.deepEqual(
      style.get(event, true, false).ranges.map((r) => [r.anchor, r.head]),
      [[2, 19]],
    );
    assert.deepEqual(
      style.get(event, false, true).ranges.map((r) => [r.anchor, r.head]),
      [
        [2, 2],
        [19, 19],
      ],
    );
    assert.deepEqual(
      style.get(heldTokenPointer(23, 1), false, false).ranges.map((r) => [r.anchor, r.head]),
      [[19, 23]],
    );
    assert.deepEqual(coordinates, [
      [19, 0],
      [23, 0],
    ]);
  });
});

test('chip double press uses its semantic start while following plain press retains actual word-hit continuation', () => {
  chipFixture((f) => {
    const { event, style } = f.press(19, 2);
    f.native.select(style, event);
    assert.deepEqual(
      [f.native.state.selection.main.anchor, f.native.state.selection.main.head],
      [6, 19],
    );
    assert.ok(f.native.state.field(heldProjection));
    assert.equal(f.domReads, 1);
    assert.deepEqual(f.coordinates, []);
    const following = f.press(12);
    f.native.select(following.style, following.event);
    assert.deepEqual(
      [f.native.state.selection.main.anchor, f.native.state.selection.main.head],
      [12, 16],
    );
    assert.equal(f.domReads, 1);
    assert.deepEqual(f.coordinates, [[12, 0]]);
  });
});
