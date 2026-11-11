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
import {
  reorderPaintStyleFixture,
  type ReorderPaintStyleFixture,
} from './reorder-paint-style-fixture.ts';
/** Passive paint tests fake only finite DOM/style/canvas boundaries; native journeys certify actual glyph layout. */
import assert from 'node:assert/strict';
import test from 'node:test';
import type { EditorView } from '@codemirror/view';
import { captureReorderGrabPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-paint.ts';

/** Exact finite external paint APIs are nominally adapted only at the EditorView boundary. */
interface PaintNode {
  localName: string;
  nodeType: number;
  className: string;
  childNodes: PaintNode[];
  parentNode: PaintNode | null;
  readonly parentElement: PaintNode | null;
  readonly isConnected: boolean;
  getRootNode(): object;
  width: number;
  height: number;
  inert: boolean;
  removed: boolean;
  style: ReorderPaintStyleFixture;
  cloneNode(deep: boolean): PaintNode;
  properties: Map<string, string>;
  attributes: Map<string, string>;
  getAttribute(name: string): string | null;
  contains(candidate: unknown): boolean;
  querySelectorAll(): PaintNode[];
  getClientRects(): {
    left: number;
    right: number;
    top: number;
    bottom: number;
    width: number;
    height: number;
  }[];
  setAttribute(name: string, value: string): void;
  append(child: PaintNode): void;
  remove(): void;
  getContext(): { drawImage(): void } | null;
  getBoundingClientRect(): {
    left: number;
    right: number;
    top: number;
    bottom: number;
    width: number;
    height: number;
  };
}

/** Literal external DOM boundary exposes allocation, copied styles and resource cleanup without a second rendering owner. */
function fixture(
  media: readonly { readonly width: number; readonly height: number }[] = [],
  context = true,
  failure?: {
    readonly paint?: unknown;
    readonly remove?: unknown;
    readonly style?: unknown;
  },
) {
  const created: PaintNode[] = [];
  let draws = 0;
  /** Construct only the finite APIs consumed by passive paint. */
  function node(localName = 'span'): PaintNode {
    const properties = new Map<string, string>(),
      style = reorderPaintStyleFixture(properties);
    if (failure && 'style' in failure)
      Object.defineProperty(style, 'cssText', {
        set(_value: string) {
          throw failure.style;
        },
      });
    return {
      localName,
      nodeType: 1,
      className: '',
      childNodes: [] as PaintNode[],
      parentNode: null,
      /** Native ancestry remains separate from the finite materialized-copy lifetime. */
      get parentElement() {
        return this.parentNode;
      },
      /** Source connectivity follows retained parents and literal removal. */
      get isConnected(): boolean {
        return this === line
          ? !this.removed
          : this.parentNode?.isConnected === true && this.parentNode.childNodes.includes(this);
      },
      /** Detached copies retain their own native subtree root instead of an editor document. */
      getRootNode(): object {
        return this.isConnected
          ? view.dom.ownerDocument
          : this.parentNode
            ? this.parentNode.getRootNode()
            : this;
      },
      width: 0,
      height: 0,
      inert: false,
      removed: false,
      style,
      properties,
      attributes: new Map<string, string>(),
      cloneNode(deep: boolean) {
        if (deep) throw new Error('controlled-paint-deep-clone-unexpected');
        const copy = node(this.localName);
        created.push(copy);
        copy.style.cssText = this.style.cssText;
        for (const [name, value] of this.properties) copy.style.setProperty(name, value);
        for (const [name, value] of this.attributes) copy.attributes.set(name, value);
        copy.className = this.className;
        return copy;
      },
      contains(candidate: unknown) {
        return candidate === this || this.childNodes.some((child) => child.contains(candidate));
      },
      querySelectorAll() {
        return this.childNodes;
      },
      getClientRects: () => [],
      getAttribute(name: string) {
        return name === 'class' ? this.className : (this.attributes.get(name) ?? null);
      },
      setAttribute(name: string, value: string) {
        this.attributes.set(name, value);
      },
      append(child: PaintNode) {
        const previous = child.parentNode?.childNodes,
          at = previous?.indexOf(child) ?? -1;
        if (at >= 0) previous!.splice(at, 1);
        child.parentNode = this;
        this.childNodes.push(child);
      },
      remove() {
        const children = this.parentNode?.childNodes,
          at = children?.indexOf(this) ?? -1;
        if (at >= 0) children!.splice(at, 1);
        this.parentNode = null;
        this.removed = true;
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (failure?.remove) throw failure.remove;
      },
      getContext() {
        return context
          ? {
              drawImage() {
                draws++;
                // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
                if (failure?.paint) throw failure.paint;
              },
            }
          : null;
      },
      getBoundingClientRect: () => ({
        left: 37,
        top: 13,
        right: 277.375,
        bottom: 32.625,
        width: 240.375,
        height: 19.625,
      }),
    };
  }
  const line = node('div'),
    held = Object.assign(node(), { closest: () => line });
  line.className = 'cm-line';
  held.className = 'sp-reorder-chip';
  held.getClientRects = () => [
    { left: 70, right: 100, top: 13, bottom: 22, width: 30, height: 9 },
    { left: 37, right: 90, top: 23, bottom: 32.625, width: 53, height: 9.625 },
  ];
  for (const dimensions of media) held.append(Object.assign(node('canvas'), dimensions));
  line.append(held);
  const view = {
    scaleX: 1,
    scaleY: 1,
    contentDOM: {
      querySelectorAll: () => [held],
      contains: (candidate: unknown) => candidate === line,
    },
    dom: {
      /** This isolated painter transports canonical roots, without a native-value publication. */
      getAttribute: () => null,
      ownerDocument: {
        createElement(name: string) {
          const createdNode = node(name);
          created.push(createdNode);
          return createdNode;
        },
        defaultView: {
          getComputedStyle: () => ({
            width: '240.375px',
            height: '19.625px',
            boxSizing: 'border-box',
            outlineStyle: 'none',
            length: 0,
            getPropertyValue: () => '',
          }),
        },
      },
    },
  } as unknown as EditorView;
  return {
    view,
    press: { x: 45, y: 27 },
    line,
    created,
    get draws() {
      return draws;
    },
  };
}

test('fractional native line dimensions preserve original layout and disjoint clipping without rounding scale', () => {
  const f = fixture(),
    paint = captureReorderGrabPaint(f.view, 0, f.press);
  assert.ok(paint);
  const proxy = f.created[0]!,
    line = proxy.childNodes[0]!;
  assert.equal((line.style as unknown as Record<string, string>).width, '240.375px');
  assert.equal((line.style as unknown as Record<string, string>).height, '19.625px');
  assert.equal((line.style as unknown as Record<string, string>).transform, 'scale(1, 1)');
  assert.equal(
    (proxy.style as unknown as Record<string, string>).clipPath,
    'path("M33 0h30v9h-30Z M0 10h53v9.625h-53Z")',
  );
  assert.equal(proxy.inert, true);
  assert.equal(proxy.attributes.get('aria-hidden'), 'true');
  assert.equal(line.attributes.size, 0);
});

test('aggregate decoded pixel budget rejects capture before any proxy allocation', () => {
  const f = fixture(Array.from({ length: 5 }, () => ({ width: 1024, height: 1024 })));
  assert.equal(captureReorderGrabPaint(f.view, 0, f.press), null);
  assert.equal(f.created.length, 0);
});

test('oversized native line text rejects before geometry or computed style reads', () => {
  const f = fixture();
  const held = f.line.childNodes[0]!;
  held.childNodes.push({
    ...held,
    nodeType: 3,
    childNodes: [],
    textContent: 'x'.repeat(16385),
  } as PaintNode);
  f.line.getBoundingClientRect = () => {
    throw new Error('geometry-before-admission');
  };
  f.view.dom.ownerDocument.defaultView!.getComputedStyle = () => {
    throw new Error('style-before-admission');
  };
  assert.equal(captureReorderGrabPaint(f.view, 0, f.press), null);
  assert.equal(f.created.length, 0);
});

test('decoded native canvas copies exact intrinsic pixels without acquiring images', () => {
  const f = fixture([{ width: 64, height: 48 }]);
  const held = f.line.childNodes[0]!;
  held.attributes.set('data-media', 'ready');
  held.attributes.set('data-resolution', 'found');
  held.attributes.set('id', 'native-input-owner');
  held.attributes.set('contenteditable', 'true');
  assert.ok(captureReorderGrabPaint(f.view, 0, f.press));
  assert.equal(f.draws, 1);
  const canvas = f.created.find((entry) => entry.localName === 'canvas')!;
  assert.deepEqual([canvas.width, canvas.height], [64, 48]);
  assert.equal(
    f.created.some((entry) => entry.localName === 'img'),
    false,
  );
  const copied = f.created.find((entry) => entry.className === 'sp-reorder-chip')!;
  assert.deepEqual(
    [...copied.attributes],
    [
      ['data-media', 'ready'],
      ['data-resolution', 'found'],
    ],
  );
});

test('unavailable canvas context rejects the whole snapshot and retires its partially created proxy', () => {
  const f = fixture([{ width: 64, height: 48 }], false);
  assert.equal(captureReorderGrabPaint(f.view, 0, f.press), null);
  assert.equal(f.created[0]!.removed, true);
  assert.equal(f.draws, 0);
});

test('native zero-paint widget buffers preserve replaced layout and child cardinality without media admission or drawing', () => {
  const f = fixture([], false);
  const buffer = Object.assign({}, f.line.childNodes[0]!, {
    localName: 'img',
    className: 'cm-widgetBuffer',
    width: 0,
    height: 14,
    naturalWidth: 0,
    naturalHeight: 0,
    complete: true,
    childNodes: [],
    attributes: new Map<string, string>(),
  });
  f.line.append(buffer);
  const paint = captureReorderGrabPaint(f.view, 0, f.press);
  assert.ok(paint);
  const copied = f.created.find((node) => node.className === 'cm-widgetBuffer')!;
  assert.equal(copied.localName, 'canvas');
  assert.deepEqual([copied.width, copied.height], [0, 0]);
  assert.equal(paint.node.childNodes[0]!.childNodes.length, f.line.childNodes.length);
  assert.equal(copied.attributes.size, 0);
  assert.equal(f.draws, 0);
});

test('ordinary zero-size unavailable host images remain rejected before paint allocation', () => {
  const f = fixture();
  f.line.append(
    Object.assign({}, f.line.childNodes[0]!, {
      localName: 'img',
      className: 'host-image',
      width: 0,
      height: 14,
      naturalWidth: 0,
      naturalHeight: 0,
      complete: true,
      childNodes: [],
    }),
  );
  assert.equal(captureReorderGrabPaint(f.view, 0, f.press), null);
  assert.equal(f.created.length, 0);
});

/** Failed canvas, containing copies, outer proxy and their shared detached template each retain their own cleanup cause. */
test('unexpected paint and cleanup failures retain every original cause in order', () => {
  const paint = new Error('controlled-paint-failure'),
    cleanup = new Error('controlled-cleanup-failure');
  const f = fixture([{ width: 64, height: 48 }], true, {
    paint,
    remove: cleanup,
  });
  assert.throws(
    () => captureReorderGrabPaint(f.view, 0, f.press),
    (error: unknown) => {
      assert.ok(error instanceof AggregateError);
      assert.equal(error.errors.length, 2);
      assert.equal(error.errors[1], cleanup);
      let primary: unknown = error.errors[0];
      for (let frontier = 0; frontier < 4; frontier++) {
        assert.ok(primary instanceof AggregateError);
        assert.equal(primary.errors.length, 2);
        assert.equal(primary.errors[1], cleanup);
        primary = primary.errors[0];
      }
      assert.equal(primary, paint);
      return true;
    },
  );
  assert.equal(f.created.length, 5);
  assert.ok(f.created.every((node) => node.removed));
});

/** Actual snapshot rollback retains a failed native batch write before root removal failure. */
test('batch CSS publication and snapshot cleanup preserve primary and release causes', () => {
  const style = new Error('controlled-batch-css-publication'),
    remove = new Error('controlled-batch-root-release'),
    f = fixture([], true, { style, remove });
  assert.throws(
    () => captureReorderGrabPaint(f.view, 0, f.press),
    (cause) =>
      cause instanceof AggregateError &&
      cause.errors.length === 2 &&
      cause.errors[0] instanceof AggregateError &&
      cause.errors[0].errors[0] === style &&
      cause.errors[0].errors[1] === remove &&
      cause.errors[0].errors.length === 2 &&
      cause.errors[1] === remove,
  );
  assert.equal(f.created[0]!.removed, true);
  assert.equal(f.created[1]!.removed, true);
});

/** Actual grab geometry and complete glyph/canvas copy cannot interleave live computed CSS with destination allocation. */
test('whole grab resolves original style before allocating its outer proxy', () => {
  const f = fixture(),
    window = f.view.dom.ownerDocument.defaultView!,
    read = window.getComputedStyle;
  let reads = 0;
  window.getComputedStyle = (...args) => {
    assert.equal(f.created.length, 0, 'grab-computed-style-after-allocation');
    const style = Reflect.apply(read, window, args);
    return new Proxy(style, {
      get(target, property, receiver) {
        if (property === 'getPropertyValue')
          return (name: string) => {
            assert.equal(f.created.length, 0, 'grab-computed-property-after-allocation');
            ++reads;
            return target.getPropertyValue(name);
          };
        return Reflect.get(target, property, receiver);
      },
    });
  };
  const paint = captureReorderGrabPaint(f.view, 0, f.press);
  assert.ok(paint);
  assert.ok(reads >= 144);
  assert.ok(f.created.length > 0);
  paint.node.remove();
});
