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
import type { ReorderLayoutReadAuthority } from '../../../../../../frontend/sugarprompt/editor/src/reorder-slot-layout-read.ts';
import { admitReorderTargetLayout as readTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
/** Request-local mounted observations prove native read reuse and authority; browser proof owns actual layout and pixels. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { ReorderMountedLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-layout.ts';
import type { MountedReorderTarget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-mounted-target.ts';
import { ReorderWidget } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget.ts';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { wholeFixture } from './reorder-pointer-stage-fixture.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { reorderMotionTargets } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-targets.ts';
/** Direct owner tests have no Slot observer; native-drain composition is proved through the actual Stage separately. */
function direct(available: () => boolean): ReorderLayoutReadAuthority {
  return { available, guard: (predicate) => predicate() };
}
/** Direct geometry-only controls deliberately own no Slot observation. */
const unobserved = direct(() => true);
/** Direct owner controls request complete publication coverage. */
const complete = { kind: 'complete' } as const;
/** Literal independent viewport operands include fractional origins and zero-width markers. */
function box(left: number, top: number, width = 12, height = 16): DOMRect {
  return {
    x: left,
    y: top,
    left,
    right: left + width,
    top,
    bottom: top + height,
    width,
    height,
    toJSON: () => ({ left, top, width, height }),
  };
}
/** Transport a native iterable list without reproducing semantic target filtering. */
function rectList(rects: DOMRect[]): DOMRectList {
  return Object.assign(rects, {
    item: (index: number) => rects[index] ?? null,
  });
}
test('one fresh READ shares all 180 root samples with rich footprints and zero-width semantic markers', () => {
  const f = motionPresentationFixture(),
    owner = new ReorderMountedLayout(f.view);
  const targets: MountedReorderTarget[] = [],
    roots: HTMLElement[] = [],
    reads: number[] = [],
    boundsReads: number[] = [];
  const line = f.root.closest<HTMLElement>('.cm-line')!;
  const window = f.view.dom.ownerDocument.defaultView!,
    computed = window.getComputedStyle;
  const mutable = box(0.3, 0.7),
    rich = box(2.3, 1.7, 3, 4);
  let armed = false;
  try {
    for (let index = 0; index < 182; index++) {
      const node = f.view.dom.ownerDocument.createElement('span');
      line.append(node);
      const geometry =
        index === 0 ? mutable : box(index + 0.3, index * 20 + 0.7, index >= 180 ? 0 : 12);
      reads.push(0);
      boundsReads.push(0);
      node.getBoundingClientRect = () => {
        if (armed && ++boundsReads[index]! > 1) throw new Error('duplicate-root-bounds-read');
        return geometry;
      };
      node.getClientRects = () => {
        if (armed && ++reads[index]! > 1) throw new Error('duplicate-root-native-read');
        return rectList([geometry]);
      };
      roots.push(node);
      targets.push({
        node,
        kind: index < 180 ? 'chip' : index === 180 ? 'gap' : 'partition',
        index,
        ...(index === 180 ? { blank: 0 } : {}),
        leases: [],
        origins: [index],
      });
    }
    const descendant = f.view.dom.ownerDocument.createElement('span');
    descendant.getClientRects = () => rectList([rich]);
    roots[0]!.append(descendant);
    const cached = owner.measure(targets);
    assert.ok(cached);
    armed = true;
    window.getComputedStyle = () => {
      throw new Error('fresh-read-discovered-style-cache-key');
    };
    const observed = owner.observe(targets, unobserved, readTargets, complete);
    assert.ok(observed);
    assert.deepEqual(observed.layout, cached);
    assert.equal(observed.layout.length, 182);
    assert.equal(observed.footprints.size, 180);
    assert.deepEqual(
      reads,
      Array.from({ length: 182 }, () => 1),
    );
    assert.deepEqual(
      boundsReads,
      Array.from({ length: 182 }, () => 1),
    );
    assert.equal(observed.footprints.get(roots[0]!)!.length, 2);
    assert.equal(observed.footprints.get(roots[179]!)![0]!.top, 3580.7);
    assert.deepEqual(
      observed.layout.slice(-2).map(({ kind, left, right, top, bottom }) => ({
        kind,
        width: right - left,
        height: bottom - top,
      })),
      [
        { kind: 'gap', width: 0, height: 16 },
        { kind: 'partition', width: 0, height: 16 },
      ],
    );
    const retained = observed.footprints.get(roots[0]!)![0]!.left;
    mutable.x = 900;
    Object.defineProperty(mutable, 'left', { value: 900 });
    assert.equal(observed.footprints.get(roots[0]!)![0]!.left, retained);
    assert.ok(Object.isFrozen(observed.layout));
    assert.ok(Object.isFrozen(observed.footprints.get(roots[0]!)![0]));
    reads.fill(0);
    boundsReads.fill(0);
    const next = owner.observe(targets, unobserved, readTargets, complete);
    assert.ok(next);
    assert.notEqual(next, observed);
    assert.equal(next.footprints.get(roots[0]!)![0]!.left, 900);
    assert.deepEqual(
      reads,
      Array.from({ length: 182 }, () => 1),
    );
    assert.deepEqual(
      boundsReads,
      Array.from({ length: 182 }, () => 1),
    );
  } finally {
    window.getComputedStyle = computed;
    owner.destroy();
    for (const root of roots) root.remove();
    f.destroy();
  }
});
for (const frontier of ['request', 'same-preview-issue', 'destroy'] as const)
  test(`fresh mounted geometry rejects ${frontier} replacement during an actual root read`, () => {
    const f = motionPresentationFixture();
    const preview = f.state.field(reorderPreview).value!;
    const paint = new ReorderWidget(preview, null).mountPassive(f.view);
    f.root.closest<HTMLElement>('.cm-line')!.append(paint.node);
    const source = readExactSource(f.state),
      selection = f.state.selection;
    let current = true;
    try {
      const targets = paint.targets(),
        first = targets.find((target) => target.kind === 'chip')!.node;
      const read = first.getClientRects;
      let replaced = false;
      first.getClientRects = () => {
        if (!replaced) {
          replaced = true;
          if (frontier === 'request') current = false;
          else if (frontier === 'same-preview-issue')
            assert.equal(paint.reconcile(preview), 'ready');
          else paint.destroy();
        }
        return read.call(first);
      };
      assert.equal(
        paint.observe(
          direct(() => current),
          readTargets,
          complete,
        ),
        null,
      );
      assert.equal(replaced, true);
      if (frontier === 'same-preview-issue') {
        first.getClientRects = read;
        assert.equal(paint.current(), true);
        assert.ok(paint.observe(unobserved, readTargets, complete));
      }
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state), 0);
      assert.equal(redoDepth(f.state), 0);
    } finally {
      paint.destroy();
      assert.equal(paint.observe(unobserved, readTargets, complete), null);
      f.destroy();
    }
  });
test('oversized native root lists reject before reading or copying any fragment', () => {
  const f = motionPresentationFixture(),
    owner = new ReorderMountedLayout(f.view);
  const node = f.view.dom.ownerDocument.createElement('span');
  f.root.closest<HTMLElement>('.cm-line')!.append(node);
  let itemReads = 0;
  try {
    const hostile = new Proxy(rectList([]), {
      get(_target, property) {
        if (property === 'length') return 4097;
        itemReads++;
        throw new Error('overbudget-native-list-item-read');
      },
    });
    node.getClientRects = () => hostile;
    assert.equal(
      owner.observe(
        [{ node, kind: 'chip', index: 0, leases: [], origins: [] }],
        unobserved,
        readTargets,
        complete,
      ),
      null,
    );
    assert.equal(itemReads, 0);
  } finally {
    owner.destroy();
    node.remove();
    f.destroy();
  }
});
for (const boundary of ['bounds', 'fragments'] as const)
  test(`retirement during native ${boundary} does not consume the returned hostile result`, () => {
    const f = motionPresentationFixture(),
      owner = new ReorderMountedLayout(f.view);
    const node = f.view.dom.ownerDocument.createElement('span');
    f.root.closest<HTMLElement>('.cm-line')!.append(node);
    let current = true,
      consumed = 0;
    try {
      const hostileBounds = new Proxy(box(1, 2), {
        get() {
          consumed++;
          throw new Error('retired-native-bounds-consumed');
        },
      });
      const hostileList = new Proxy(rectList([]), {
        get() {
          consumed++;
          throw new Error('retired-native-list-consumed');
        },
      });
      if (boundary === 'bounds')
        node.getBoundingClientRect = () => {
          current = false;
          return hostileBounds;
        };
      else
        node.getClientRects = () => {
          current = false;
          return hostileList;
        };
      assert.equal(
        owner.observe(
          [{ node, kind: 'chip', index: 0, leases: [], origins: [] }],
          direct(() => current),
          readTargets,
          complete,
        ),
        null,
      );
      assert.equal(consumed, 0);
    } finally {
      owner.destroy();
      node.remove();
      f.destroy();
    }
  });
for (const cause of [undefined, null, new Error('native-root-read-failed')])
  test('fresh observation preserves raw native read failure and exact later retirement', () => {
    const f = motionPresentationFixture(),
      preview = f.state.field(reorderPreview).value!;
    const paint = new ReorderWidget(preview, null).mountPassive(f.view);
    f.root.closest<HTMLElement>('.cm-line')!.append(paint.node);
    const source = readExactSource(f.state),
      selection = f.state.selection;
    try {
      paint.targets().find((target) => target.kind === 'chip')!.node.getClientRects = () => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw cause;
      };
      let failed = false;
      try {
        paint.observe(unobserved, readTargets, complete);
      } catch (actual) {
        failed = true;
        assert.equal(actual, cause);
      }
      assert.equal(failed, true);
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state) + redoDepth(f.state), 0);
    } finally {
      paint.destroy();
      assert.equal(paint.current(), false);
      assert.equal(paint.observe(unobserved, readTargets, complete), null);
      f.destroy();
    }
  });
for (const invalid of [
  'line-width',
  'root-height',
  'fragment-width',
  'fragment-coordinate',
] as const)
  test(`fresh observations retain the native ${invalid} admission guard`, () => {
    const f = motionPresentationFixture(),
      owner = new ReorderMountedLayout(f.view);
    const line = f.root.closest<HTMLElement>('.cm-line')!,
      node = f.view.dom.ownerDocument.createElement('span');
    const lineBounds = line.getBoundingClientRect;
    line.append(node);
    try {
      node.getBoundingClientRect = () => box(1, 2, 12, invalid === 'root-height' ? 0 : 16);
      node.getClientRects = () =>
        rectList([
          box(
            invalid === 'fragment-coordinate' ? NaN : 1,
            2,
            invalid === 'fragment-width' ? -1 : 12,
          ),
        ]);
      if (invalid === 'line-width') line.getBoundingClientRect = () => box(0, 0, 0);
      assert.equal(
        owner.observe(
          [{ node, kind: 'chip', index: 0, leases: [], origins: [] }],
          unobserved,
          readTargets,
          complete,
        ),
        null,
      );
    } finally {
      line.getBoundingClientRect = lineBounds;
      owner.destroy();
      node.remove();
      f.destroy();
    }
  });
test('whole Stage consumes one fresh mounted observation per cold and warm Pending READ without the cached measure path', () => {
  const f = wholeFixture(),
    measure = ReorderMountedLayout.prototype.measure;
  let stage: ReorderPassiveStage | null = null;
  const source = readExactSource(f.state),
    selection = f.state.selection;
  try {
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared?.whole);
    stage = new ReorderPassiveStage(f.view, prepared);
    const child = stage.mount(prepared);
    assert.ok(child);
    f.paint.admitLayer(stage);
    const select = (geometry: Parameters<typeof reorderMotionTargets>[0]) =>
      reorderMotionTargets(geometry, geometry, null);
    const placement = child.read(select);
    assert.equal(placement.kind, 'placement');
    if (placement.kind !== 'placement') assert.fail('native-placement-required');
    child.place(placement);
    const roots = Array.from(child.node.querySelectorAll<HTMLElement>('[data-reorder-chip]'));
    assert.equal(roots.length, 3);
    const reads = roots.map(() => 0);
    for (const [index, root] of roots.entries()) {
      const original = root.getClientRects;
      root.getClientRects = () => {
        if (++reads[index]! > 1) throw new Error('whole-stage-duplicate-root-read');
        return original.call(root);
      };
    }
    ReorderMountedLayout.prototype.measure = () => {
      throw new Error('whole-stage-used-cross-frame-measure');
    };
    const ready = child.read(select);
    assert.equal(ready.kind, 'ready');
    if (ready.kind !== 'ready') assert.fail('whole-stage-ready-required');
    assert.equal(ready.scene.whole, true);
    assert.equal(new Set(ready.scene.geometry.map((rect) => rect.index)).size, 3);
    assert.ok(ready.scene.layout?.length);
    assert.deepEqual(reads, [1, 1, 1]);
    child.destroy();
    const warm = stage.mount(prepared);
    assert.ok(warm);
    reads.fill(0);
    assert.equal(warm.node, child.node);
    const renewed = warm.read(select);
    assert.equal(renewed.kind, 'ready');
    if (renewed.kind !== 'ready') assert.fail('warm-whole-stage-ready-required');
    assert.notEqual(renewed.scene.layout, ready.scene.layout);
    assert.deepEqual(reads, [1, 1, 1]);
    assert.equal(readExactSource(f.state), source);
    assert.equal(f.state.selection, selection);
    assert.equal(undoDepth(f.state) + redoDepth(f.state), 0);
    stage.destroy();
    assert.equal(child.node.isConnected, false);
    assert.equal(warm.node.isConnected, false);
    assert.equal(stage.node.isConnected, false);
  } finally {
    ReorderMountedLayout.prototype.measure = measure;
    stage?.destroy();
    f.destroy();
  }
});
for (const mutation of ['appearance', 'scroll', 'window', 'publication', 'stage-destroy'] as const)
  test(`whole observation rechecks native ${mutation} after a root callback before selecting or publishing`, () => {
    const f = motionPresentationFixture();
    let stage: ReorderPassiveStage | null = null;
    const source = readExactSource(f.state),
      selection = f.state.selection;
    try {
      const captured = preparePassiveMotion(f.view, 0);
      assert.ok(captured);
      const prepared = { ...captured, whole: true as const };
      stage = new ReorderPassiveStage(f.view, prepared);
      const child = stage.mount(prepared);
      assert.ok(child);
      f.paint.admitLayer(stage);
      const select = (geometry: Parameters<typeof reorderMotionTargets>[0]) =>
        reorderMotionTargets(geometry, geometry, null);
      const placement = child.read(select);
      if (placement.kind !== 'placement') assert.fail('native-placement-required');
      child.place(placement);
      const root = child.node.querySelector<HTMLElement>('[data-reorder-chip]')!;
      const read = root.getClientRects;
      root.getClientRects = () => {
        if (mutation === 'appearance')
          f.view.dom.setAttribute('data-observation-reentry', 'changed');
        else if (mutation === 'scroll') f.view.scrollDOM.scrollTop++;
        else if (mutation === 'publication') f.publish();
        else if (mutation === 'stage-destroy') stage!.destroy();
        else
          Object.defineProperty(f.view.dom.ownerDocument.defaultView!, 'innerWidth', {
            configurable: true,
            value: (f.view.dom.ownerDocument.defaultView!.innerWidth ?? 0) + 1,
          });
        return read.call(root);
      };
      assert.equal(
        child.read(() => {
          assert.fail('stale-appearance-reached-selection');
        }).kind,
        'unavailable',
      );
      assert.equal(readExactSource(f.state), source);
      assert.equal(f.state.selection, selection);
      assert.equal(undoDepth(f.state) + redoDepth(f.state), 0);
      assert.equal(f.state.field(reorderPreview).mask, null);
      if (mutation === 'stage-destroy') {
        assert.equal(child.node.isConnected, false);
        assert.equal(stage.node.isConnected, false);
        assert.equal(f.paint.layerMount.style.width, '0px');
        assert.equal(f.paint.layerMount.style.height, '0px');
      }
    } finally {
      f.view.dom.removeAttribute('data-observation-reentry');
      stage?.destroy();
      f.destroy();
    }
  });
