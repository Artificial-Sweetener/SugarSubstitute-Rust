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
/** Actual collision ownership consumes literal native fragments; this proof does not emulate browser containment or certify wrapping. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { hideReorderChip } from '../../../../../../frontend/sugarprompt/core/src/index.ts';
import { ReorderCollisionBase } from '../../../../../../frontend/sugarprompt/editor/src/reorder-collision-base.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { fixture, key } from './reorder-session-fixture.ts';
import { collisionGeometry } from './reorder-passive-geometry-fixture.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';

for (const top of [300, 400])
  test(`fixed256 collision calibration retains full canonical targets at nativeY${top}`, () => {
    const f = fixture(),
      base = new ReorderCollisionBase(f.view),
      causes: unknown[] = [];
    try {
      f.owner.keydown(key('Alt'));
      f.measure();
      f.task();
      const preview = f.state.field(reorderPreview).value;
      assert.ok(preview);
      const hidden = hideReorderChip(preview.capture.prepared, preview.order, preview.activeChip);
      assert.ok(hidden);
      const input = {
        token: preview.token,
        capture: preview.capture,
        order: preview.order,
        hidden,
        held: preview.activeChip,
        pointer: 7,
        coordinates: { x: 15, y: 20 },
      };
      f.view.scrollDOM.getBoundingClientRect = () =>
        ({
          left: 0,
          right: 200,
          top: 0,
          bottom: 500,
          width: 200,
          height: 500,
        }) as DOMRect;
      collisionGeometry(f, (id) => [
        {
          left: id === 1 ? 20 : 60,
          right: id === 1 ? 40 : 80,
          top,
          bottom: top + 20,
          width: 20,
          height: 20,
        } as DOMRect,
      ]);
      const source = readExactSource(f.state),
        selection = f.state.selection;
      assert.equal(base.prepare(input), true);
      const context = base.contextAt(input);
      assert.ok(context);
      base.mount(input, context);
      const root = Array.from(f.view.dom.ownerDocument.body.childNodes).find(
        (node) =>
          node.nodeType === 1 &&
          (node as HTMLElement).className.includes('sp-reorder-base-measurement'),
      ) as HTMLElement | undefined;
      assert.ok(root);
      assert.equal(root.style.height, '256px');
      assert.equal(root.style.contain, 'layout size');
      assert.equal(root.style.overflow, 'visible');
      const calibration = base.measure(input, context);
      assert.equal(calibration.kind, 'calibration');
      if (calibration.kind !== 'calibration') assert.fail('collision-calibration-required');
      assert.equal(calibration.scaleX, 1);
      assert.equal(calibration.scaleY, 1);
      assert.equal(base.align(calibration), true);
      assert.equal(root.style.height, '256px');
      const measured = base.measure(input, context);
      assert.equal(measured.kind, 'ready');
      if (measured.kind !== 'ready') assert.fail('complete-collision-output-required');
      const chips = measured.rects.filter((rect) => rect.kind === 'chip');
      assert.deepEqual(chips.map((rect) => rect.index).sort(), [1, 2]);
      assert.ok(chips.every((rect) => rect.top === top && rect.bottom === top + 20));
      assert.ok(chips.every((rect) => rect.top > root.getBoundingClientRect().bottom));
      const paint = root.childNodes[0]!.childNodes[0]!.childNodes[0]!.childNodes[1]!;
      assert.equal(
        Array.from(paint.childNodes).filter(
          (node) => node.nodeType === 1 && (node as HTMLElement).dataset.reorderChip !== undefined,
        ).length,
        2,
      );
      const topology = base.placements(input, context);
      assert.ok(topology);
      assert.deepEqual(base.target(input, context, 75, top + 10), {
        kind: 'row',
        row: 0,
        insertion: 2,
      });
      assert.equal(base.current(input, context), measured.rects);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
      assert.equal(redoDepth(f.state), 0);
      base.destroy();
      assert.equal(root.isConnected, false);
    } catch (cause) {
      causes.push(cause);
    } finally {
      for (const release of [() => base.destroy(), () => f.destroy()])
        try {
          release();
        } catch (cause) {
          causes.push(cause);
        }
    }
    if (causes.length === 1) throw causes[0];
    if (causes.length) throw new AggregateError(causes, 'collision-isolation-proof-failed');
  });

/** Actual external collision paint has a replaceable native containing line and independently delivered font events. */
function renewalFixture() {
  const f = motionPresentationFixture(),
    base = new ReorderCollisionBase(f.view),
    fonts = new Map<string, () => void>();
  Object.defineProperty(f.paint.document, 'createRange', {
    /** Collapsed LTR caret readings use only the existing independently supplied native target rectangles. */
    value() {
      let start: Node | null = null,
        offset = 0;
      return {
        setStart(node: Node, at: number): void {
          start = node;
          offset = at;
        },
        collapse(): void {},
        getClientRects() {
          let target = start;
          while (
            target &&
            !(
              target.nodeType === 1 &&
              ['reorderChip', 'reorderGap', 'reorderPartition'].some(
                (name) => (target as HTMLElement).dataset[name] !== undefined,
              )
            )
          )
            target = target.parentNode;
          if (!target && start) {
            const parent = start.nodeType === 3 ? start.parentNode : start;
            target =
              Array.from(parent?.childNodes ?? []).find(
                (node) =>
                  node.nodeType === 1 &&
                  ['reorderGap', 'reorderPartition'].some(
                    (name) => (node as HTMLElement).dataset[name] !== undefined,
                  ),
              ) ?? null;
          }
          assert.ok(target?.nodeType === 1);
          const box = (target as HTMLElement).getBoundingClientRect(),
            x = offset === 0 ? box.left : box.right;
          // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
          return [{ ...box, left: x, right: x, width: 0 }];
        },
      };
    },
  });
  Object.defineProperty(f.paint.document, 'fonts', {
    value: {
      addEventListener: (name: string, callback: () => void) => fonts.set(name, callback),
      removeEventListener: (name: string) => fonts.delete(name),
    },
  });
  try {
    const hidden = hideReorderChip(f.input.capture.prepared, f.input.order, 0);
    assert.ok(hidden);
    const input = { ...f.input, hidden, pointer: 7 };
    assert.equal(base.prepare(input), true);
    const context = base.contextAt(input);
    assert.ok(context);
    base.mount(input, context);
    const calibration = base.measure(input, context);
    assert.equal(calibration.kind, 'calibration');
    if (calibration.kind !== 'calibration') assert.fail('collision-calibration-required');
    assert.equal(base.align(calibration), true);
    const measured = base.measure(input, context);
    assert.equal(measured.kind, 'ready');
    if (measured.kind !== 'ready') assert.fail('collision-geometry-required');
    assert.deepEqual(
      measured.rects
        .filter((rect) => rect.kind === 'chip')
        .map((rect) => [rect.index, rect.left, rect.right, rect.top, rect.bottom])
        .sort((a, b) => a[0]! - b[0]!),
      [
        [1, 20, 40, 20, 40],
        [2, 100, 120, 20, 40],
      ],
    );
    const root = calibration.root,
      paint = root.querySelector<HTMLElement>('.sp-reorder-preview'),
      targets = Array.from(root.querySelectorAll<HTMLElement>('[data-reorder-chip]')),
      fragments = targets.flatMap((target) => Array.from(target.childNodes));
    assert.ok(paint && targets.length === 2 && fragments.length > 1);
    return {
      f,
      base,
      input,
      context,
      root,
      paint,
      targets,
      fragments,
      fonts,
      rects: measured.rects,
      /** Replace only the controlled native line; all original canonical output remains in its actual backing tree. */
      replace() {
        const line = f.paint.document.createElement('div');
        line.className = f.paint.line.className;
        Object.assign(line.box, f.paint.line.box);
        line.append(...f.paint.line.childNodes.slice());
        f.paint.content.append(line);
        f.paint.line.remove();
        f.view.domAtPos = () => ({ node: line as unknown as Node, offset: 0 });
        return line;
      },
      /** Both actual finite owners retire independently, preserving the original failure first. */
      destroy() {
        const causes: unknown[] = [];
        for (const release of [() => base.destroy(), () => f.destroy()])
          try {
            release();
          } catch (cause) {
            causes.push(cause);
          }
        if (causes.length === 1) throw causes[0];
        if (causes.length) throw new AggregateError(causes, 'collision-renewal-retirement');
      },
    };
  } catch (primary) {
    const causes: unknown[] = [primary];
    for (const release of [() => base.destroy(), () => f.destroy()])
      try {
        release();
      } catch (cause) {
        causes.push(cause);
      }
    if (causes.length > 1) throw new AggregateError(causes, 'collision-renewal-acquisition');
    throw primary;
  }
}

test('equivalent native-line replacement renews only in WRITE and retains actual collision fragments through the following READ', () => {
  const x = renewalFixture();
  try {
    const state = x.f.state;
    const parent = x.f.paint.document.createElement('span'),
      direct = x.f.paint.document.createTextNode('native');
    parent.append(direct);
    assert.equal(direct.data === direct.textContent, true);
    assert.equal(direct.parentNode === parent && direct.parentElement === parent, true);
    direct.remove();
    assert.equal(direct.parentNode === null && direct.parentElement === null, true);
    parent.textContent = 'native';
    const assigned = parent.childNodes[0];
    assert.ok(assigned?.nodeType === 3);
    assert.equal(assigned.data === assigned.textContent, true);
    assert.equal(assigned.parentNode === parent && assigned.parentElement === parent, true);
    parent.remove();
    x.replace();
    const next = x.base.contextAt(x.input);
    assert.ok(next);
    assert.equal(x.context.current(), false);
    assert.equal(x.context.appearanceCurrent(), true);
    assert.equal(next.current(), true);
    assert.equal(next.key === x.context.key, true);
    assert.equal(next.lineParent === x.context.lineParent, true);
    assert.equal(next.lineRoot === x.context.lineRoot, true);
    assert.equal(x.base.current(x.input, next) === null, true);
    assert.equal(x.base.measure(x.input, next).kind, 'unavailable');
    const allocations = x.f.paint.allocations;
    x.base.mount(x.input, next);
    assert.equal(x.f.paint.allocations, allocations);
    assert.equal(x.root.isConnected, true);
    assert.equal(x.base.current(x.input, next) === null, true);
    const read = x.base.measure(x.input, next);
    assert.equal(read.kind, 'ready');
    if (read.kind !== 'ready') assert.fail('renewed-collision-geometry-required');
    assert.deepEqual(
      read.rects.map((rect) => [rect.kind, rect.index]),
      x.rects.map((rect) => [rect.kind, rect.index]),
    );
    const targets = Array.from(x.root.querySelectorAll<HTMLElement>('[data-reorder-chip]'));
    assert.equal(targets.length, x.targets.length);
    assert.equal(
      targets.every((node, index) => node === x.targets[index]),
      true,
    );
    const fragments = targets.flatMap((target) => Array.from(target.childNodes));
    assert.equal(fragments.length, x.fragments.length);
    assert.equal(
      fragments.every((node, index) => node === x.fragments[index]),
      true,
    );
    assert.equal(x.base.current(x.input, next) === read.rects, true);
    assert.equal(x.f.state === state, true);
  } finally {
    x.destroy();
  }
});

for (const changed of [
  'ancestor',
  'style',
  'width',
  'font',
  'font-ready',
  'resource',
  'root',
  'fresh-stamp',
  'native-root',
] as const)
  test(`native-line renewal refuses old collision geometry after ${changed}`, () => {
    const x = renewalFixture();
    try {
      const line = x.replace(),
        window = x.f.paint.document.defaultView,
        computed = window.getComputedStyle;
      if (changed === 'ancestor') x.f.paint.content.style.padding = '1px';
      if (changed === 'width') Object.assign(line.box, { width: line.box.width + 10 });
      if (changed === 'style' || changed === 'font')
        window.getComputedStyle = (node) => {
          const style = computed(node);
          return {
            ...style,
            getPropertyValue: (name: string) =>
              node === line && name === (changed === 'font' ? 'font' : 'letter-spacing')
                ? changed === 'font'
                  ? '18px serif'
                  : '2px'
                : style.getPropertyValue(name),
          };
        };
      if (changed === 'font-ready') {
        const callback = x.fonts.get('loadingdone');
        assert.ok(callback);
        callback();
      }
      if (changed === 'resource')
        new ReorderWidget(x.f.state.field(reorderPreview).value!, null).destroy(x.paint);
      if (changed === 'root') x.root.remove();
      if (changed === 'native-root') line.getRootNode = () => x.f.paint.document.body!;
      const next = x.base.contextAt(x.input);
      assert.ok(next);
      if (changed === 'fresh-stamp') line.style.padding = '1px';
      assert.equal(x.base.current(x.input, next) === null, true);
      assert.equal(x.base.measure(x.input, next).kind, 'unavailable');
      x.base.mount(x.input, next);
      assert.equal(x.root.isConnected, false);
      if (changed === 'fresh-stamp')
        assert.equal(x.base.measure(x.input, next).kind, 'unavailable');
      else assert.equal(x.base.measure(x.input, next).kind, 'calibration');
    } finally {
      x.destroy();
    }
  });
