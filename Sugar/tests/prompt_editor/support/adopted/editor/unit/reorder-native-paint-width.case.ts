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
/** Literal CSSOM rounding and DOMRect operands prove used-width transport without pretending to raster native glyphs. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  admitNativeReorderPaint,
  nativeReorderLineSize,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-paint.ts';
import { prepareNativeReorderPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-preparation.ts';
import { readReorderBaseContext } from '../../../../../../frontend/sugarprompt/editor/src/reorder-base-context.ts';
import { nativeFixture } from './reorder-native-projection-fixture.ts';
import { NativeReorderAxes } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-axes.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

/** Every new proof attempts its finite layer cleanup after preserving an original raw assertion/callback cause. */
function proof(body: (fixture: ReturnType<typeof motionPaintFixture>) => void): void {
  const f = motionPaintFixture(),
    causes: unknown[] = [];
  try {
    body(f);
  } catch (cause) {
    causes.push(cause);
  }
  try {
    f.destroyLayer();
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'native-width-proof-cleanup');
}
/** Only literal computed-style operands are changed; the actual production serializer/preparation remains under test. */
function styles(
  f: ReturnType<typeof motionPaintFixture>,
  selected: typeof f.line,
  values: Readonly<Record<string, string>>,
) {
  const original = f.document.defaultView.getComputedStyle;
  f.document.defaultView.getComputedStyle = (node, pseudo) => {
    const base = original(node, pseudo);
    return node !== selected
      ? base
      : {
          ...base,
          ...values,
          getPropertyValue: (name: string) => values[name] ?? base.getPropertyValue(name),
        };
  };
}
/** The actual immutable descriptor is observed before any native destination allocation. */
function prepared(f: ReturnType<typeof motionPaintFixture>) {
  const nodes = admitNativeReorderPaint([f.line as unknown as HTMLElement]);
  assert.ok(nodes);
  const result = prepareNativeReorderPaint(f.view, [f.line as unknown as Node], nodes, 'copy');
  assert.equal(f.allocations, 0);
  return result;
}
for (const [serialized, precise] of [
  ['228.031px', 228.03125],
  ['91.1406px', 91.140625],
  ['12.3281px', 12.328125],
  ['52.7812px', 52.78125],
] as const)
  test(`native used width retains the independent full-precision operand ${precise}`, () =>
    proof((f) => {
      const node = f.chips[1]!;
      Object.assign(node.box, { width: precise });
      styles(f, node, {
        width: serialized,
        display: 'inline-flex',
        'box-sizing': 'border-box',
      });
      const result = prepared(f),
        descriptor = result?.get(node as unknown as Node);
      assert.ok(descriptor?.kind === 'element');
      assert.equal(descriptor.cssText.includes(`width:${precise}px;`), true);
      assert.equal(descriptor.cssText.includes(`width:${serialized};`), false);
      assert.equal(f.line.textContent, 'abc');
    }));

test('content-box width removes its actual transported padding and borders without an integer offsetWidth', () =>
  proof((f) => {
    const node = f.chips[1]!;
    Object.assign(node.box, { width: 108.03125 });
    styles(f, node, {
      width: '100.031px',
      display: 'block',
      'box-sizing': 'content-box',
      'padding-left': '2.5px',
      'padding-right': '3.5px',
      'border-left-width': '1px',
      'border-right-width': '1px',
    });
    Object.defineProperty(node, 'offsetWidth', {
      get() {
        throw new Error('integer-width-used');
      },
    });
    const descriptor = prepared(f)?.get(node as unknown as Node);
    assert.ok(descriptor?.kind === 'element');
    assert.equal(descriptor.cssText.includes('width:100.03125px;'), true);
  }));

for (const display of ['inline', 'none', 'contents'] as const)
  test(`native ${display} non-box width remains its literal resolved declaration`, () =>
    proof((f) => {
      const node = f.chips[1]!;
      styles(f, node, { width: '12.3281px', display });
      node.getBoundingClientRect = () => {
        throw new Error('ignored-width-measured');
      };
      const descriptor = prepared(f)?.get(node as unknown as Node);
      assert.ok(descriptor?.kind === 'element');
      assert.equal(descriptor.cssText.includes('width:12.3281px;'), true);
    }));

test('native used-width read refuses synchronous source replacement before consuming returned geometry', () =>
  proof((f) => {
    const node = f.chips[1]!;
    styles(f, node, {
      width: '228.031px',
      display: 'inline-flex',
      'box-sizing': 'border-box',
    });
    node.getBoundingClientRect = () => {
      Object.defineProperty(f.view, 'state', { value: {}, configurable: true });
      return {
        get width(): never {
          throw new Error('retired-result-consumed');
        },
      } as unknown as DOMRect;
    };
    assert.equal(prepared(f) === null, true);
  }));

test('native used-width read rejects an ancestor style mutation without an EditorState publication', () =>
  proof((f) => {
    const node = f.chips[1]!,
      read = node.getBoundingClientRect.bind(node);
    styles(f, node, {
      width: '228.031px',
      display: 'inline-flex',
      'box-sizing': 'border-box',
    });
    node.getBoundingClientRect = () => {
      f.editor.style.setProperty('color', 'red');
      return read();
    };
    assert.equal(prepared(f) === null, true);
  }));

for (const cause of [null, undefined])
  test(`native width getter preserves raw ${String(cause)} before allocation`, () =>
    proof((f) => {
      const node = f.chips[1]!;
      styles(f, node, {
        width: '228.031px',
        display: 'inline-flex',
        'box-sizing': 'border-box',
      });
      node.getBoundingClientRect = () => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw cause;
      };
      assert.throws(
        () => prepared(f),
        (error) => error === cause,
      );
      assert.equal(f.allocations, 0);
    }));
/** The literal matrix transport returns independently declared native axes; no source matrix parser is copied. */
for (const compensated of [false, true])
  test(`native and inverse Stage axes retain width independently of view.scale (${compensated})`, () =>
    proof((f) => {
      const global = globalThis as {
          DOMMatrixReadOnly?: typeof DOMMatrixReadOnly;
        },
        previous = global.DOMMatrixReadOnly,
        failures: unknown[] = [];
      let matrices = 0;
      class Matrix {
        readonly is2D = true;
        readonly b = 0;
        readonly c = 0;
        readonly a: number;
        readonly d: number;
        constructor(value: string) {
          matrices++;
          assert.ok(
            value === 'matrix(1.25, 0, 0, 1.5, 0, 0)' || value === 'matrix(0.8, 0, 0, 0.5, 0, 0)',
          );
          this.a = value === 'matrix(1.25, 0, 0, 1.5, 0, 0)' ? 1.25 : 0.8;
          this.d = value === 'matrix(1.25, 0, 0, 1.5, 0, 0)' ? 1.5 : 0.5;
        }
      }
      global.DOMMatrixReadOnly = Matrix as unknown as typeof DOMMatrixReadOnly;
      try {
        const node = f.chips[1]!;
        Object.assign(node.box, {
          width: compensated ? 228.03125 : 285.0390625,
        });
        Object.defineProperty(f.view, 'scaleX', { value: 1.25 });
        styles(f, f.editor, { transform: 'matrix(1.25, 0, 0, 1.5, 0, 0)' });
        if (compensated) styles(f, f.content, { transform: 'matrix(0.8, 0, 0, 0.5, 0, 0)' });
        styles(f, node, {
          width: '228.031px',
          display: 'inline-flex',
          'box-sizing': 'border-box',
        });
        const original = f.document.defaultView.getComputedStyle,
          reads = new Map<object, number>();
        f.document.defaultView.getComputedStyle = (element, pseudo) => {
          reads.set(element, (reads.get(element) ?? 0) + 1);
          return original(element, pseudo);
        };
        const descriptor = prepared(f)?.get(node as unknown as Node);
        assert.ok(descriptor?.kind === 'element');
        assert.equal(descriptor.cssText.includes('width:228.03125px;'), true);
        assert.equal(matrices, compensated ? 2 : 1);
        assert.equal(
          [...reads.values()].every((count) => count === 1),
          true,
        );
      } catch (cause) {
        failures.push(cause);
      }
      try {
        if (previous) global.DOMMatrixReadOnly = previous;
        else delete global.DOMMatrixReadOnly;
      } catch (cause) {
        failures.push(cause);
      }
      if (failures.length === 1) throw failures[0];
      if (failures.length) throw new AggregateError(failures, 'native-matrix-proof-cleanup');
    }));

/** Existing grab consumers must not overwrite precise descriptors using a scale derived from rounded line CSS. */
test('native line publication dimensions retain exact border-box precision', () =>
  proof((f) => {
    Object.assign(f.line.box, { width: 228.03125 });
    styles(f, f.line, {
      width: '228.031px',
      display: 'block',
      'box-sizing': 'border-box',
    });
    const size = nativeReorderLineSize(f.view, f.line as unknown as HTMLElement);
    assert.deepEqual(size, { width: 228.03125, height: 20 });
    assert.equal(f.line.getBoundingClientRect().width / size.width, 1);
  }));

/** The containing context and copied line both retain border-box precision and actual CSS sizing space. */
for (const sizing of ['border-box', 'content-box'] as const)
  test(`native containing context retains precise ${sizing} width and height`, () => {
    const t = nativeFixture(),
      failures: unknown[] = [];
    try {
      Object.assign(t.f.line.box, { width: 228.03125, height: 20.03125 });
      styles(t.f, t.f.line, {
        width: sizing === 'border-box' ? '228.031px' : '222.031px',
        height: '20.031px',
        'box-sizing': sizing,
        'padding-left': '2px',
        'padding-right': '2px',
        'border-left-width': '1px',
        'border-right-width': '1px',
      });
      const context = readReorderBaseContext(t.f.view, t.pickup);
      assert.ok(context);
      assert.equal(context.width, 228.03125);
      assert.equal(context.height, 20.03125);
      assert.equal(
        context.line.find(([name]) => name === 'width')?.[1],
        sizing === 'border-box' ? '228.03125px' : '222.03125px',
      );
    } catch (cause) {
      failures.push(cause);
    }
    try {
      t.owner.destroy();
    } catch (cause) {
      failures.push(cause);
    }
    try {
      t.f.destroyLayer();
    } catch (cause) {
      failures.push(cause);
    }
    if (failures.length === 1) throw failures[0];
    if (failures.length) throw new AggregateError(failures, 'native-context-width-proof-cleanup');
  });

/** SVGAnimatedString identity stays constant while the literal native attribute can change. */
test('native axis freshness rejects a mutated SVG class behind a stable animated object', () =>
  proof((f) => {
    const node = f.chips[1]!,
      animated = { baseVal: 'before', animVal: 'before' };
    Object.defineProperty(node, 'className', { value: animated });
    const original = node.getAttribute.bind(node);
    node.getAttribute = (name) => (name === 'class' ? animated.baseVal : original(name));
    const axes = new NativeReorderAxes(f.document.defaultView as unknown as Window, () => true);
    assert.ok(axes.read(node as unknown as HTMLElement));
    animated.baseVal = 'after';
    assert.equal(axes.current(), false);
  }));

/** The final delegated literal getter cannot retire source and return an otherwise matching stale stamp. */
test('native line publication rejects source cancellation from the last literal style getter', () =>
  proof((f) => {
    const node = f.line,
      original = node.getAttribute.bind(node);
    let armed = false,
      cancellations = 0;
    const bounds = node.getBoundingClientRect.bind(node);
    node.getBoundingClientRect = () => {
      const result = bounds();
      armed = true;
      return result;
    };
    node.getAttribute = (name) => {
      const value = original(name);
      if (name === 'style' && armed) {
        armed = false;
        cancellations++;
        Object.defineProperty(f.view, 'state', { value: {} });
      }
      return value;
    };
    assert.equal(nativeReorderLineSize(f.view, node as unknown as HTMLElement), null);
    assert.equal(cancellations, 1);
  }));

/** Construction's last returned DOMRect scalar remains inside the exact READ issue. */
test('native containing context rejects cancellation from its final returned top getter', () => {
  const t = nativeFixture(),
    failures: unknown[] = [];
  let reads = 0,
    cancelled = false;
  try {
    const bounds = t.f.line.getBoundingClientRect.bind(t.f.line);
    t.f.line.getBoundingClientRect = () => ({
      // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
      ...bounds(),
      get top() {
        if (++reads === 3) {
          cancelled = true;
          t.f.view.dispatch({ selection: { anchor: 0 } });
        }
        return 20;
      },
    });
    assert.equal(readReorderBaseContext(t.f.view, t.pickup) === null, true);
    assert.equal(cancelled, true);
  } catch (cause) {
    failures.push(cause);
  }
  try {
    t.owner.destroy();
  } catch (cause) {
    failures.push(cause);
  }
  try {
    t.f.destroyLayer();
  } catch (cause) {
    failures.push(cause);
  }
  if (failures.length === 1) throw failures[0];
  if (failures.length)
    throw new AggregateError(failures, 'native-context-final-read-proof-cleanup');
});

/** Unrepresented native scaling cannot masquerade as a larger local containing line. */
test('native containing and publication dimensions retain the independent original box mismatch refusal', () => {
  const t = nativeFixture(),
    failures: unknown[] = [];
  try {
    Object.assign(t.f.line.box, { width: 250 });
    styles(t.f, t.f.line, {
      width: '200px',
      height: '20px',
      'box-sizing': 'border-box',
    });
    assert.equal(readReorderBaseContext(t.f.view, t.pickup) === null, true);
    assert.equal(nativeReorderLineSize(t.f.view, t.f.line as unknown as HTMLElement), null);
  } catch (cause) {
    failures.push(cause);
  }
  try {
    t.owner.destroy();
  } catch (cause) {
    failures.push(cause);
  }
  try {
    t.f.destroyLayer();
  } catch (cause) {
    failures.push(cause);
  }
  if (failures.length === 1) throw failures[0];
  if (failures.length) throw new AggregateError(failures, 'native-context-mismatch-proof-cleanup');
});
