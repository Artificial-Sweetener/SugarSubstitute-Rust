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
import { nativeReorderFootprint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-footprint.ts';
import { admitReorderLineLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
import type { ReorderRect } from '../../../../../../frontend/sugarprompt/editor/src/reorder-geometry.ts';
/** Native snapshots retain source paint independently of the canonical motion scene. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { captureNativePaint } from './reorder-native-paint-fixture.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

test('actual native snapshots retain literal soft/hard breaks, readonly identity and partial opacity', () => {
  const f = motionPaintFixture();
  f.chips[1]!.append(f.document.createElement('wbr'));
  f.chips[1]!.append(f.document.createElement('br'));
  f.chips[1]!.style.opacity = '0.73';
  f.chips[1]!.setAttribute('contenteditable', 'false');
  const copy = captureNativePaint(f);
  assert.ok(copy);
  try {
    const node = copy.querySelectorAll<HTMLElement>('.sp-reorder-chip')[1];
    assert.ok(node);
    assert.equal(node.contentEditable, 'false');
    assert.equal(node.style.opacity, '0.73');
    assert.deepEqual(
      Array.from(node.childNodes)
        .filter((n) => n.nodeType === 1)
        .map((n) => (n as Element).localName),
      ['wbr', 'br'],
    );
    assert.equal(f.line.textContent, 'abc');
  } finally {
    copy.remove();
  }
});
for (const excessive of ['nodes', 'characters'] as const)
  test(`native snapshot ${excessive} budget refuses before destination allocation`, () => {
    const f = motionPaintFixture();
    if (excessive === 'nodes') for (let i = 0; i < 1024; i++) f.line.append(f.text('x'));
    else f.chips[0]!.append(f.text('x'.repeat(16385)));
    assert.equal(captureNativePaint(f), null);
    assert.equal(f.allocations, 0);
  });
test('native CharacterData length refuses before reading an oversized text value', () => {
  const f = motionPaintFixture(),
    text = f.text('x');
  Object.defineProperty(text, 'length', { value: 16385 });
  Object.defineProperty(text, 'textContent', {
    get() {
      throw new Error('unbounded-text-read');
    },
  });
  f.line.append(text);
  assert.equal(captureNativePaint(f), null);
  assert.equal(f.allocations, 0);
});
test('unavailable native decoded media refuses complete snapshots before allocation', () => {
  const f = motionPaintFixture();
  Object.assign(f.chips[1]!, {
    localName: 'img',
    complete: false,
    naturalWidth: 12,
    naturalHeight: 12,
    width: 12,
  });
  assert.equal(captureNativePaint(f), null);
  assert.equal(f.allocations, 0);
});
test('native SVG arrow coordinates survive actual safe snapshot copying', () => {
  const f = motionPaintFixture(),
    svg = f.document.createElementNS('http://www.w3.org/2000/svg', 'svg'),
    triangle = f.document.createElementNS('http://www.w3.org/2000/svg', 'polygon');
  Object.assign(svg.box, f.chips[1]!.box);
  Object.assign(triangle.box, f.chips[1]!.box);
  svg.setAttribute('viewBox', '0 0 13 10');
  svg.setAttribute('width', '100%');
  svg.setAttribute('height', '100%');
  triangle.setAttribute('points', '6.5,3 10,7 3,7');
  triangle.setAttribute('fill', 'currentColor');
  svg.append(triangle);
  f.chips[1]!.append(svg);
  const copy = captureNativePaint(f);
  assert.ok(copy);
  try {
    const copied = copy.querySelector('svg');
    assert.ok(copied);
    assert.equal(copied.namespaceURI, 'http://www.w3.org/2000/svg');
    assert.equal(copied.getAttribute('viewBox'), '0 0 13 10');
    const polygon = copied.childNodes[0] as Element;
    assert.equal(polygon.getAttribute('points'), '6.5,3 10,7 3,7');
    assert.equal(polygon.getAttribute('fill'), 'currentColor');
    assert.equal(Reflect.get((polygon as SVGElement).style, 'pointer-events'), 'none');
  } finally {
    copy.remove();
  }
});

/** Native canvas calls are the literal external boundary; prepared media and transfer are the real production owners. */
function pixels(
  f: ReturnType<typeof motionPaintFixture>,
  failure: { readonly cause: unknown } | null = null,
  unavailable = false,
) {
  let count = 0;
  const create = f.document.createElement.bind(f.document);
  f.document.createElement = (name) => {
    const node = create(name);
    if (name === 'canvas')
      Object.assign(node, {
        width: 4,
        height: 3,
        getContext: () =>
          unavailable
            ? null
            : {
                drawImage() {
                  count++;
                  if (failure) throw failure.cause;
                },
              },
      });
    return node;
  };
  const canvas = f.document.createElement('canvas');
  Object.assign(canvas.box, f.chips[1]!.box);
  f.chips[1]!.append(canvas);
  return () => count;
}
test('actual prepared native media transfers its literal pixels once with the complete readonly snapshot', () => {
  const f = motionPaintFixture(),
    count = pixels(f);
  const copy = captureNativePaint(f);
  assert.ok(copy);
  try {
    assert.equal(count(), 1);
    assert.equal(copy.textContent, 'abc');
  } finally {
    copy.remove();
  }
});
for (const primary of [null, undefined, new Error('controlled-native-media')] as const)
  for (const cleanupFails of [false, true])
    test(`native media refusal retains raw primary ${String(primary)} cleanup=${cleanupFails}`, () => {
      const f = motionPaintFixture(),
        cleanup = new Error('controlled-native-copy-cleanup');
      pixels(f, { cause: primary });
      const create = f.document.createElement.bind(f.document);
      let removed = 0;
      f.document.createElement = (name) => {
        const node = create(name),
          remove = node.remove.bind(node);
        node.remove = () => {
          remove();
          if (node.className === 'cm-line') {
            removed++;
            if (cleanupFails) throw cleanup;
          }
        };
        return node;
      };
      let failed = false,
        actual: unknown;
      try {
        captureNativePaint(f);
      } catch (cause) {
        failed = true;
        actual = cause;
      }
      assert.equal(failed, true);
      if (cleanupFails) {
        assert.ok(actual instanceof AggregateError);
        assert.deepEqual(actual.errors, [primary, cleanup]);
      } else assert.equal(actual, primary);
      assert.equal(removed, 1);
      assert.equal(f.line.textContent, 'abc');
      assert.equal(f.frames.size, 0);
    });
test('unavailable native media context rejects complete snapshot output', () => {
  const f = motionPaintFixture();
  pixels(f, null, true);
  assert.equal(captureNativePaint(f), null);
  assert.equal(f.line.textContent, 'abc');
  assert.equal(f.frames.size, 0);
});

const completeViewport = { left: 0, right: 200, top: 0, bottom: 100 };
/** Independent complete-peer contract accepts every observed positive root/descendant fragment or none. */
function completePeer(rects: readonly ReorderRect[] | null): readonly ReorderRect[] | null {
  return rects?.length &&
    rects.every(
      (rect) => rect.left >= 0 && rect.right <= 200 && rect.top >= 0 && rect.bottom <= 100,
    )
    ? rects
    : null;
}

for (const edge of ['left', 'right', 'top', 'bottom', 'wrapped'] as const)
  test(`positive native root ${edge} fragment disqualifies the complete peer before hostile descendant reads`, () => {
    const f = motionPaintFixture(),
      root = f.chips[0]!,
      child = f.document.createElement('span');
    root.append(child);
    const inside = root.getBoundingClientRect(),
      outside = {
        // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
        ...inside,
        ...(edge === 'left'
          ? { left: -1, right: 19 }
          : edge === 'right'
            ? { left: 190, right: 210 }
            : edge === 'top'
              ? { top: -1, bottom: 19 }
              : { top: 90, bottom: 110 }),
      };
    root.getClientRects = () => (edge === 'wrapped' ? [inside, outside] : [outside]);
    Object.assign(child.box, { left: 20, top: 20, width: 20, height: 20 });
    const element = root as unknown as HTMLElement,
      nodes = admitReorderLineLayout([element]);
    assert.ok(nodes);
    // The existing complete-output rule already rejected this actual native root, even with visible child paint.
    assert.equal(completePeer(nativeReorderFootprint(element, nodes, 0)), null);
    let rootReads = 0;
    const observeRoot = root.getClientRects.bind(root);
    root.getClientRects = () => {
      rootReads++;
      return observeRoot();
    };
    child.getClientRects = () => {
      assert.fail('disqualified-offscreen-descendant-read');
    };
    assert.equal(nativeReorderFootprint(element, nodes, 0, completeViewport), null);
    assert.equal(rootReads, 1);
    assert.equal(f.line.textContent, 'abc');
    assert.equal(f.frames.size, 0);
  });

for (const rootKind of ['inside', 'zero', 'empty'] as const)
  test(`native ${rootKind} root retains actual positive rich descendants and the exact uncullled output`, () => {
    const f = motionPaintFixture(),
      root = f.chips[0]!,
      child = f.document.createElement('span');
    root.append(child);
    Object.assign(child.box, { left: 15, top: 15, width: 30, height: 30 });
    const inside = root.getBoundingClientRect();
    root.getClientRects = () =>
      rootKind === 'inside'
        ? [inside]
        : rootKind === 'empty'
          ? []
          : // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
            [{ ...inside, width: 0, height: 0 }];
    const element = root as unknown as HTMLElement,
      nodes = admitReorderLineLayout([element]);
    assert.ok(nodes);
    const expected = nativeReorderFootprint(element, nodes, 0);
    assert.ok(expected);
    assert.ok(completePeer(expected));
    let childReads = 0,
      rootReads = 0;
    const readChild = child.getClientRects.bind(child),
      readRoot = root.getClientRects.bind(root);
    child.getClientRects = () => {
      childReads++;
      return readChild();
    };
    root.getClientRects = () => {
      rootReads++;
      return readRoot();
    };
    assert.deepEqual(nativeReorderFootprint(element, nodes, 0, completeViewport), expected);
    assert.equal(childReads, 1);
    assert.equal(rootReads, 1);
  });

test('inside root rich overflow is still observed and rejects complete-peer admission; held capture retains the same actual pieces', () => {
  const f = motionPaintFixture(),
    root = f.chips[0]!,
    child = f.document.createElement('span');
  root.append(child);
  Object.assign(child.box, { left: -10, top: 20, width: 30, height: 20 });
  const element = root as unknown as HTMLElement,
    nodes = admitReorderLineLayout([element]);
  assert.ok(nodes);
  const expected = nativeReorderFootprint(element, nodes, 0);
  assert.ok(expected);
  assert.equal(completePeer(expected), null);
  let reads = 0;
  const read = child.getClientRects.bind(child);
  child.getClientRects = () => {
    reads++;
    return read();
  };
  const actual = nativeReorderFootprint(element, nodes, 0, completeViewport);
  assert.deepEqual(actual, expected);
  assert.equal(reads, 1);
  assert.equal(completePeer(actual), null);
  assert.deepEqual(nativeReorderFootprint(element, nodes, 0), expected);
  assert.equal(reads, 2);
});

test('root admission retains the finite fragment bound and observable unexpected native failures', () => {
  const f = motionPaintFixture(),
    root = f.chips[0]!,
    element = root as unknown as HTMLElement,
    nodes = admitReorderLineLayout([element]);
  assert.ok(nodes);
  const box = root.getBoundingClientRect();
  root.getClientRects = () => Array.from({ length: 4097 }, () => box);
  assert.equal(nativeReorderFootprint(element, nodes, 0, completeViewport), null);
  // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
  root.getClientRects = () => [{ ...box, left: NaN }];
  assert.equal(nativeReorderFootprint(element, nodes, 0, completeViewport), null);
  const cause = new Error('controlled-native-root-fragment-read');
  root.getClientRects = () => {
    throw cause;
  };
  assert.throws(
    () => nativeReorderFootprint(element, nodes, 0, completeViewport),
    (error) => error === cause,
  );
});
