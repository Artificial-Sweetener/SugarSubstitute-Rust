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
/** Sparse geometry prepares hidden carriers only; the same genuine Pending must publish complete native Scene geometry. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import { readReorderMotionOrigin } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-origin.ts';
import {
  borrowDisplayedReorderGrab,
  captureDisplayedReorderGrab,
  hitCurrentDisplayedReorderPaint,
  readCurrentDisplayedReorderPaint,
  type ReorderDisplayedPaintPorts,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-displayed-paint.ts';
import {
  usingCandidateReadFixture,
  selectCandidatePeer,
  candidateRects,
} from './reorder-candidate-read-fixture.ts';
import { admitReorderLineLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
import type { ReorderMotionScene } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-scene.ts';

/** Complete literal native operands compare independently mounted owners without source-bearing scope/function identities. */
function output(scene: ReorderMotionScene) {
  return {
    geometry: scene.geometry,
    layout: scene.layout,
    targets: scene.targets,
    endpoint: scene.endpoint && {
      held: scene.endpoint.held,
      viewport: scene.endpoint.viewport,
      color: scene.endpoint.color,
      order: scene.endpoint.order.order,
      partitions: scene.endpoint.order.partitions,
    },
    chips: scene.chips.map((chip) => ({
      id: chip.id,
      rects: chip.rects,
      left: chip.left,
      top: chip.top,
      roots: chip.roots.map(({ scaleX, scaleY }) => ({ scaleX, scaleY })),
    })),
  };
}

/** The actual native carrier request has one exact existing WRITE before a complete validating READ. */
function createHidden(f: Parameters<Parameters<typeof usingCandidateReadFixture>[0]>[0]): void {
  const read = f.child.read(selectCandidatePeer);
  assert.equal(read.kind, 'carriers');
  if (read.kind !== 'carriers') assert.fail('candidate-carrier-create-required');
  assert.equal(read.operation, 'create');
  assert.deepEqual(f.coverages, ['carrier-candidates']);
  assert.equal(Reflect.has(read, 'scene'), false);
  assert.equal(f.child.configureCarriers(read.operation), true);
  const copies = f.child.node.querySelectorAll<HTMLElement>('[data-reorder-motion-carrier]');
  assert.equal(copies.length, 1);
  assert.equal(copies[0]!.style.opacity, '0');
  for (const root of f.roots.values()) assert.equal(root.dataset.reorderMotionFlow, undefined);
}

test('only first carrier preparation omits clipped rich paint; final Scene, held contour, static hit/origin and paused whole grab stay complete', () => {
  let baseline: ReturnType<typeof output> | null = null;
  usingCandidateReadFixture(
    (f) => {
      const first = f.child.read(selectCandidatePeer);
      assert.equal(first.kind, 'carriers');
      if (first.kind !== 'carriers') assert.fail('complete-baseline-carriers-required');
      assert.equal(first.operation, 'create');
      assert.equal(f.child.configureCarriers(first.operation), true);
      const read = f.child.read(selectCandidatePeer);
      assert.equal(read.kind, 'ready');
      if (read.kind !== 'ready') assert.fail('complete-baseline-scene-required');
      assert.deepEqual(f.coverages, ['complete', 'complete']);
      assert.equal(read.scene.endpoint?.order, f.prepared.preview.order);
      baseline = output(read.scene);
    },
    { complete: true },
  );
  usingCandidateReadFixture((f) => {
    const source = readExactSource(f.f.state),
      selection = f.f.state.selection;
    for (const id of [0, 2]) {
      const node = f.rich.get(id)!,
        native = node.getClientRects;
      node.getClientRects = () => {
        if (f.phase() === 'carrier-candidates')
          assert.fail('sparse-read-discovered-ineligible-rich-fragments');
        return native.call(node);
      };
    }
    createHidden(f);
    assert.deepEqual(Array.from(f.rootReads.entries()).sort(), [
      [0, 1],
      [1, 1],
      [2, 1],
    ]);
    assert.equal(f.richReads.get(0) ?? 0, 0);
    assert.equal(f.richReads.get(2) ?? 0, 0);
    assert.equal(f.richReads.get(1), 1);
    const read = f.child.read(selectCandidatePeer);
    assert.equal(read.kind, 'ready');
    if (read.kind !== 'ready') assert.fail('complete-candidate-scene-required');
    const scene = read.scene;
    assert.deepEqual(output(scene), baseline);
    assert.deepEqual(f.coverages, ['carrier-candidates', 'complete']);
    assert.equal(scene.endpoint?.order, f.prepared.preview.order);
    assert.deepEqual([...new Set(scene.geometry.map((rect) => rect.index))].sort(), [0, 1, 2]);
    assert.deepEqual(
      scene.targets.map((target) => target.id),
      [1],
    );
    assert.equal(
      scene.geometry.some((rect) => rect.index === 0 && rect.left < 0),
      true,
    );
    assert.equal(
      scene.geometry.some((rect) => rect.index === 2 && rect.right > 240),
      true,
    );
    assert.deepEqual(Array.from(f.rootReads.entries()).sort(), [
      [0, 2],
      [1, 2],
      [2, 2],
    ]);
    assert.equal(f.richReads.get(0), 1);
    assert.equal(f.richReads.get(2), 1);
    const frame = { progress: 1, positions: [] },
      accepted = {
        preview: f.prepared.preview,
        input: f.f.input,
        current: () => f.child.current(),
        rects: scene.geometry,
      },
      ports: ReorderDisplayedPaintPorts = {
        live: () => true,
        scene: () => scene,
        frame: () => frame,
        input: () => f.f.input,
        accepted: () => accepted,
        current: () => f.child.current(),
        resourceCurrent: () => f.child.retained(),
      };
    const display = readCurrentDisplayedReorderPaint(f.f.view, ports, f.f.input);
    assert.ok(display);
    assert.equal(display.geometry, scene.layout);
    assert.deepEqual(
      display.rects,
      scene.geometry.filter((rect) => rect.index === 0),
    );
    assert.equal(
      display.rects.some((rect) => rect.left < 0),
      true,
    );
    assert.equal(hitCurrentDisplayedReorderPaint(f.f.view, ports, 235, 25), 2);
    const origin = readReorderMotionOrigin(
      f.f.view,
      { current: () => true, rects: scene.geometry },
      scene,
      frame,
    );
    assert.equal(
      origin.some((rect) => rect.index === 2 && rect.right > 240),
      true,
    );
    const grab = borrowDisplayedReorderGrab(f.f.view, ports, 2, {
      x: 235,
      y: 25,
    });
    assert.ok(grab);
    assert.equal(grab.geometry.left, 230);
    assert.equal(grab.geometry.width, 20);
    assert.equal(grab.geometry.grabX, 5);
    assert.equal(grab.current(), true);
    const captured = captureDisplayedReorderGrab(f.f.view, ports, 2, {
      held: 2,
      scene: grab.scene,
      geometry: grab.geometry,
    });
    assert.ok(captured);
    assert.equal(captured.kind, 'paint');
    if (captured.kind !== 'paint') assert.fail('complete-native-grab-copy-required');
    f.restore(() => captured.paint.node.remove());
    assert.equal(
      captured.paint.node.querySelectorAll('.sp-lora-chip').length,
      scene.node.querySelectorAll('.sp-lora-chip').length,
    );
    assert.equal(readExactSource(f.f.state), source);
    assert.equal(f.f.state.selection, selection);
    assert.equal(undoDepth(f.f.state) + redoDepth(f.f.state), 0);
  });
});

test('scroll-only viewport revocation prevents same-READ omitted-rich completion before native discovery', () => {
  usingCandidateReadFixture(
    (f) => {
      let missing = 0;
      for (const id of [0, 2])
        f.rich.get(id)!.getClientRects = () => {
          missing++;
          assert.fail('viewport-revoked-rich-read');
        };
      const read = f.child.read((geometry) => {
        f.f.view.scrollDOM.scrollLeft++;
        return selectCandidatePeer(geometry);
      });
      assert.equal(read.kind, 'unavailable');
      assert.deepEqual(f.coverages, ['carrier-candidates']);
      assert.equal(missing, 0);
      assert.equal(f.child.node.querySelectorAll('[data-reorder-motion-carrier]').length, 0);
    },
    { atomic: false },
  );
});

test('no actual atomic carrier falls back to complete rich observation in the same READ without a new waiting phase', () => {
  usingCandidateReadFixture(
    (f) => {
      let selections = 0;
      const issued: {
        geometry: Parameters<typeof selectCandidatePeer>[0] | null;
        plan: ReturnType<typeof selectCandidatePeer> | null;
      } = { geometry: null, plan: null };
      const read = f.child.read((geometry) => {
        assert.equal(++selections, 1);
        issued.geometry = geometry;
        issued.plan = selectCandidatePeer(geometry);
        return issued.plan;
      });
      assert.equal(read.kind, 'ready');
      if (read.kind !== 'ready') assert.fail('same-read-full-canonical-scene-required');
      assert.deepEqual(f.coverages, ['carrier-candidates', 'complete']);
      assert.equal(selections, 1);
      assert.equal(read.scene.targets, issued.plan);
      assert.deepEqual(
        issued.geometry,
        read.scene.geometry.filter((rect) => rect.index === 1),
      );
      assert.equal(f.child.node.querySelectorAll('[data-reorder-motion-carrier]').length, 0);
      assert.equal(
        read.scene.geometry.some((rect) => rect.index === 2),
        true,
      );
      assert.equal(f.richReads.get(2), 1);
    },
    { atomic: false },
  );
});

for (const invalidation of ['state', 'viewport'] as const)
  test(`same-READ omitted-rich ${invalidation} revocation rejects the already selected plan before Scene publication`, () => {
    usingCandidateReadFixture(
      (f) => {
        const node = f.rich.get(0)!,
          native = node.getClientRects;
        let selections = 0,
          changed = false;
        node.getClientRects = () => {
          if (!changed) {
            changed = true;
            if (invalidation === 'state') f.f.view.dispatch({ selection: { anchor: 1 } });
            else f.f.view.scrollDOM.scrollLeft++;
          }
          return native.call(node);
        };
        const read = f.child.read((geometry) => {
          assert.equal(++selections, 1);
          return selectCandidatePeer(geometry);
        });
        assert.equal(read.kind, 'unavailable');
        assert.equal(selections, 1);
        assert.equal(changed, true);
        assert.deepEqual(f.coverages, ['carrier-candidates', 'complete']);
        assert.equal(f.child.node.querySelectorAll('[data-reorder-motion-carrier]').length, 0);
        for (const root of f.roots.values())
          assert.equal(root.dataset.reorderMotionFlow, undefined);
      },
      { atomic: false },
    );
  });

test('zero viewport candidates still publish only the same-READ complete static geometry', () => {
  usingCandidateReadFixture((f) => {
    const root = f.roots.get(1)!,
      original = root.getClientRects;
    root.getClientRects = () =>
      candidateRects(
        Array.from(original.call(root), (rect) => ({
          // eslint-disable-next-line @typescript-eslint/no-misused-spread -- Native boundary fixtures intentionally project own fields without a browser class prototype.
          ...rect,
          left: 260,
          right: 280,
          x: 260,
        })),
      );
    const read = f.child.read(selectCandidatePeer);
    assert.equal(read.kind, 'ready');
    if (read.kind !== 'ready') assert.fail('empty-candidate-complete-scene-required');
    assert.deepEqual(f.coverages, ['carrier-candidates', 'complete']);
    assert.deepEqual(read.scene.targets, []);
    assert.deepEqual([...new Set(read.scene.geometry.map((rect) => rect.index))].sort(), [0, 1, 2]);
  });
});

test('overbudget whole media retains the original complete observation and viewport hydration outcome', () => {
  usingCandidateReadFixture(
    (f) => {
      const read = f.child.read(selectCandidatePeer);
      assert.equal(read.kind, 'media');
      assert.deepEqual(f.coverages, ['complete']);
      assert.ok((f.richReads.get(2) ?? 0) > 0);
      assert.equal(f.child.node.querySelectorAll('[data-reorder-motion-carrier]').length, 0);
    },
    { viewportMedia: true },
  );
});

for (const failure of ['per-root', 'aggregate', 'raw-null', 'raw-undefined', 'raw-error'] as const)
  test(`hidden carrier ownership rolls back independently after complete ${failure} rich rejection`, () => {
    usingCandidateReadFixture(
      (f) => {
        const source = readExactSource(f.f.state),
          selection = f.f.state.selection,
          raw =
            failure === 'raw-null'
              ? null
              : failure === 'raw-undefined'
                ? undefined
                : new Error('final-rich-native-failure');
        if (failure === 'aggregate') {
          for (const [id, root] of f.roots) {
            if (id === 1) continue;
            const nodes = admitReorderLineLayout([root]);
            assert.ok(nodes);
            const elements = nodes.filter((node) => node.nodeType === 1).length;
            // Each other element has one positive native rectangle; this input fills exactly the256-fragment group cap.
            const island = f.rich.get(id)!,
              original = island.getClientRects,
              count = 256 - (elements - 1);
            assert.ok(count > 0);
            island.getClientRects = () => {
              if (f.phase() !== 'complete')
                assert.fail('overbudget-rich-group-read-before-complete-validation');
              return candidateRects(Array.from({ length: count }, () => original.call(island)[0]!));
            };
          }
        } else {
          const node = f.rich.get(2)!,
            original = node.getClientRects;
          node.getClientRects = () => {
            if (f.phase() !== 'complete')
              assert.fail('ineligible-rich-read-before-complete-validation');
            if (failure === 'per-root')
              return candidateRects(Array.from({ length: 257 }, () => original.call(node)[0]!));
            // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
            throw raw;
          };
        }
        createHidden(f);
        let outcome: ReturnType<typeof f.child.read> | null = null,
          caught = false,
          cause: unknown;
        try {
          outcome = f.child.read(selectCandidatePeer);
        } catch (error) {
          caught = true;
          cause = error;
        }
        if (failure === 'per-root' || failure === 'aggregate') {
          assert.equal(caught, false);
          assert.equal(outcome?.kind, 'unavailable');
        } else {
          assert.equal(caught, true);
          assert.equal(cause, raw);
        }
        assert.deepEqual(f.coverages, ['carrier-candidates', 'complete']);
        for (const root of f.roots.values())
          assert.equal(root.dataset.reorderMotionFlow, undefined);
        f.child.destroy();
        assert.equal(f.child.node.querySelectorAll('[data-reorder-motion-carrier]').length, 0);
        assert.equal(readExactSource(f.f.state), source);
        assert.equal(f.f.state.selection, selection);
        assert.equal(undoDepth(f.f.state) + redoDepth(f.f.state), 0);
      },
      failure === 'aggregate' ? { count: 20 } : {},
    );
  });
