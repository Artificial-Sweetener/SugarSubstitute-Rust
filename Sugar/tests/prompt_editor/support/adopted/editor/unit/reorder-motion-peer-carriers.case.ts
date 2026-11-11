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
/** Actual native-copy output, canonical fragment renderer and literal viewport operands characterize the whole-Scene carrier boundary. */
import { admitReorderTargetLayout } from '../../../../../../frontend/sugarprompt/editor/src/reorder-layout-admission.ts';
import assert from 'node:assert/strict';
import test from 'node:test';
import { Decoration } from '@codemirror/view';
import { usingAtomicPaintFixture, atomicScenePaintSource } from './reorder-atomic-paint-fixture.ts';
import { admitNativeReorderPaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-native-paint.ts';
import {
  prepareNativeReorderPaint,
  preparedAtomicPaintRole,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-paint-preparation.ts';
import { atomicReorderPaintIsland } from '../../../../../../frontend/sugarprompt/editor/src/reorder-atomic-paint-island.ts';
import {
  renderReorderFragment,
  releaseReorderFragment,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-projection.ts';
import { ReorderMotionPeerCarriers } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-peer-carriers.ts';
import { ReorderScenePaint } from '../../../../../../frontend/sugarprompt/editor/src/reorder-scene-motion-paint.ts';
import { captureReorderMotionGrab } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-scene.ts';
import { reorderGrabGeometry } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-geometry.ts';
import { reorderState, captureReorder } from '../transaction/reorder-fixture.ts';
import { prepareReorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview-preparation.ts';
import { prepareReorderWidgetPlan } from '../../../../../../frontend/sugarprompt/editor/src/reorder-widget-plan.ts';

test('actual heavy canonical plan borrows 180 source fragments without Scene or regional row-edge roles', () => {
  const group = '(soft light:2.00), <lora:real:0.70>, <lora:missing:0.50>, subject, ';
  const state = reorderState(group.repeat(45)).state;
  const capture = captureReorder(state);
  const preview = prepareReorderPreview(
    { token: {}, capture, activeChip: 1, order: capture.prepared.initial },
    'complete',
  );
  assert.ok(preview);
  const plan = prepareReorderWidgetPlan(preview);
  const chips = plan.blocks.filter((block) => block.kind === 'chip');
  assert.equal(chips.length, 180);
  assert.equal(plan.nativeRowPaint, false);
  for (const chip of chips) {
    assert.equal(chip.parts.length, 1);
    const part = chip.parts[0]!;
    assert.equal(part.kind, 'line');
    if (part.kind === 'line') assert.equal(part.rowPaint, null);
  }
});

for (const rowPaint of [null, { sceneFill: true, region: null }] as const)
  test(`actual readonly borrowed-line renderer ${rowPaint ? 'retains row-edge refusal' : 'admits one complete LoRA island'}`, () =>
    usingAtomicPaintFixture((f) => {
      const root = f.roots[0]!,
        lease = f.f.document.createElement('span');
      root.replaceChildren(lease);
      const prior = f.f.document.defaultView.getComputedStyle;
      f.f.document.defaultView.getComputedStyle = (node, pseudo) => {
        const style = prior(node, pseudo);
        if (!node.classList.contains('sp-canonical-row-edge')) return style;
        return {
          ...style,
          display: 'inline-block',
          getPropertyValue: (name: string) =>
            name === 'display' ? 'inline-block' : style.getPropertyValue(name),
        };
      };
      try {
        renderReorderFragment(
          f.view,
          lease as unknown as HTMLElement,
          () => '',
          { from: 0, to: 7 },
          Decoration.set([Decoration.replace({ widget: f.widgets[0]!.widget }).range(0, 7)]),
          rowPaint,
        );
        assert.equal(lease.querySelectorAll('.sp-lora-chip').length, 1);
        assert.equal(lease.querySelectorAll('.sp-canonical-row-edge').length, rowPaint ? 2 : 0);
        const nodes = admitNativeReorderPaint([root as unknown as HTMLElement]);
        assert.ok(nodes);
        const prepared = prepareNativeReorderPaint(
          f.view,
          [root as unknown as HTMLElement],
          nodes,
          'atomic-grab',
        );
        assert.ok(prepared);
        assert.equal(
          atomicReorderPaintIsland(root as unknown as Node, (node) =>
            preparedAtomicPaintRole(prepared.get(node)),
          ),
          rowPaint ? null : lease.querySelector('.sp-lora-chip'),
        );
      } finally {
        releaseReorderFragment(lease as unknown as HTMLElement);
      }
    }));

test('actual Scene paint reuses complete native copies without touching canonical atomic offsets and retains mixed inline fallback', () =>
  usingAtomicPaintFixture((f) => {
    f.roots[1]!.append(f.f.text('outside'));
    f.roots[0]!.style.setProperty('opacity', '0.65', 'important');
    const chips = [atomicScenePaintSource(f, 0), atomicScenePaintSource(f, 1)];
    const before = chips.map((chip) => chip.rects.map((rect) => ({ ...rect })));
    const owner = ReorderMotionPeerCarriers.prepare(
      f.view,
      f.f.line as unknown as HTMLElement,
      chips,
      1,
      1,
      () => true,
    );
    assert.ok(owner);
    const paint = new ReorderScenePaint(chips, owner);
    try {
      assert.equal(
        owner.read(chips, () => true, admitReorderTargetLayout),
        'create',
      );
      owner.configure('create', () => true);
      assert.equal(
        owner.read(chips, () => true, admitReorderTargetLayout),
        'ready',
      );
      assert.equal(owner.handles(0), true);
      assert.equal(owner.handles(1), false);
      const canonical = f.roots[0]!,
        nativeStyle = canonical.style;
      Object.defineProperty(nativeStyle, 'left', {
        configurable: true,
        get: () => '',
        set: () => {
          throw new Error('canonical atomic left mutated');
        },
      });
      Object.defineProperty(nativeStyle, 'top', {
        configurable: true,
        get: () => '',
        set: () => {
          throw new Error('canonical atomic top mutated');
        },
      });
      paint.reveal(() => true);
      paint.paint(
        {
          progress: 0.5,
          positions: [
            { id: 0, left: 31, top: 26 },
            { id: 1, left: 33, top: 28 },
          ],
        },
        () => true,
      );
      const carriers = f.f.line.querySelectorAll('.sp-reorder-motion-peer');
      assert.equal(carriers.length, 1);
      assert.equal(carriers[0]!.style.transform, 'translate(11px, 6px)');
      assert.equal(canonical.style.getPropertyValue('opacity'), '0');
      assert.equal(carriers[0]!.querySelectorAll('canvas').length, 1);
      assert.equal(carriers[0]!.querySelectorAll('.sp-lora-weight').length, 1);
      assert.equal(f.roots[1]!.style.left, '13px');
      assert.equal(f.roots[1]!.style.top, '8px');
      assert.deepEqual(
        chips.map((chip) => chip.rects),
        before,
      );
      assert.deepEqual(
        chips.map((chip) => [chip.left, chip.top]),
        [
          [20, 20],
          [20, 20],
        ],
      );
    } finally {
      f.causes.push(...paint.close(() => true));
    }
    assert.equal(f.f.line.querySelectorAll('.sp-reorder-motion-peer').length, 0);
    assert.equal(f.roots[0]!.style.getPropertyValue('opacity'), '0.65');
    assert.equal(f.roots[0]!.style.getPropertyPriority('opacity'), 'important');
    assert.equal(f.roots[0]!.getAttribute('data-reorder-motion-flow'), null);
  }));

for (const boundary of ['allocation', 'append', 'opacity'] as const)
  test(`actual carrier ${boundary} retirement disposes late output and restores exact canonical paint`, () =>
    usingAtomicPaintFixture((f) => {
      const chips = [atomicScenePaintSource(f, 0)],
        root = f.roots[0]!;
      const owner = ReorderMotionPeerCarriers.prepare(
        f.view,
        f.f.line as unknown as HTMLElement,
        chips,
        1,
        1,
        () => true,
      );
      assert.ok(owner);
      try {
        if (boundary === 'allocation') {
          const create = f.f.document.createElement.bind(f.f.document);
          let once = true;
          f.f.document.createElement = (name) => {
            if (once) {
              once = false;
              owner.destroy();
            }
            return create(name);
          };
          assert.equal(
            owner.instantiate(() => true),
            false,
          );
        } else if (boundary === 'append') {
          const append = f.f.line.append.bind(f.f.line);
          let once = true;
          f.f.line.append = (...nodes) => {
            if (once) {
              once = false;
              owner.destroy();
            }
            append(...nodes);
          };
          assert.equal(
            owner.instantiate(() => true),
            false,
          );
        } else {
          assert.equal(
            owner.instantiate(() => true),
            true,
          );
          assert.equal(
            owner.read(chips, () => true, admitReorderTargetLayout),
            'ready',
          );
          const set = root.style.setProperty.bind(root.style);
          let once = true;
          root.style.setProperty = (name, value, priority) => {
            if (once && name === 'opacity' && value === '0') {
              once = false;
              owner.destroy();
            }
            set(name, value, priority);
          };
          assert.equal(
            owner.reveal(() => true),
            false,
          );
        }
        assert.equal(f.f.line.querySelectorAll('.sp-reorder-motion-peer').length, 0);
        assert.equal(root.style.getPropertyValue('opacity'), '');
        assert.equal(root.getAttribute('data-reorder-motion-flow'), null);
        assert.deepEqual(
          [root.box.left, root.box.top, root.box.width, root.box.height],
          [20, 20, 40, 20],
        );
      } finally {
        owner.destroy();
      }
    }));

test('carrier validation fences each delegated footprint read and preserves raw native failure', () =>
  usingAtomicPaintFixture((f) => {
    const chips = [atomicScenePaintSource(f, 0)];
    const owner = ReorderMotionPeerCarriers.prepare(
      f.view,
      f.f.line as unknown as HTMLElement,
      chips,
      1,
      1,
      () => true,
    );
    assert.ok(owner);
    try {
      assert.equal(
        owner.instantiate(() => true),
        true,
      );
      const root = f.f.line.querySelector('.sp-reorder-motion-peer')!.firstElementChild!;
      const failure = { code: 'literal-rect-getter' };
      root.getClientRects = () => {
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw failure;
      };
      assert.throws(
        () => owner.read(chips, () => true, admitReorderTargetLayout),
        (cause) => cause === failure,
      );
    } finally {
      owner.destroy();
    }
  }));

test('whole-scene paused capture retains original full-shell copy caps and explicit native fallback with 45 copied ready canvases', () =>
  usingAtomicPaintFixture((f) => {
    const chips = f.roots.map((_, id) => atomicScenePaintSource(f, id));
    assert.ok(admitNativeReorderPaint([f.f.line as unknown as HTMLElement]));
    const owner = ReorderMotionPeerCarriers.prepare(
      f.view,
      f.f.line as unknown as HTMLElement,
      chips,
      1,
      1,
      () => true,
    );
    assert.ok(owner);
    try {
      assert.equal(
        owner.instantiate(() => true),
        true,
      );
      assert.equal(f.f.line.querySelectorAll('.sp-reorder-motion-peer').length, 45);
      assert.equal(f.f.line.querySelectorAll('canvas').length, 90);
      assert.equal(f.draws.length, 45);
      for (let id = 0; id < 45; id++) assert.equal(f.draws[id]!.source, f.canvases[id]);
      const geometry = reorderGrabGeometry(chips[0]!.rects, 0, {
        x: 20,
        y: 20,
      });
      assert.ok(geometry);
      assert.equal(
        captureReorderMotionGrab(f.view, { node: f.f.line as unknown as HTMLElement }, geometry),
        null,
      );
      assert.ok(f.roots.every((root) => root.isConnected));
    } finally {
      owner.destroy();
    }
    assert.equal(f.f.line.querySelectorAll('canvas').length, 45);
    assert.equal(f.f.line.querySelectorAll('.sp-reorder-motion-peer').length, 0);
  }));

test('carrier footprint stops before native-list operands after a delegated read revokes the current request', () =>
  usingAtomicPaintFixture((f) => {
    const chips = [atomicScenePaintSource(f, 0)];
    const owner = ReorderMotionPeerCarriers.prepare(
      f.view,
      f.f.line as unknown as HTMLElement,
      chips,
      1,
      1,
      () => true,
    );
    assert.ok(owner);
    try {
      assert.equal(
        owner.instantiate(() => true),
        true,
      );
      const root = f.f.line.querySelector('.sp-reorder-motion-peer')!.firstElementChild!;
      let current = true,
        lengths = 0;
      const list: DOMRect[] = [];
      const native = new Proxy(list, {
        get(target, key, receiver) {
          if (key === 'length') {
            lengths++;
            throw new Error('stale rect-list length read');
          }
          return Reflect.get(target, key, receiver);
        },
      });
      root.getClientRects = () => {
        current = false;
        return native;
      };
      assert.equal(
        owner.read(chips, () => current, admitReorderTargetLayout),
        'unavailable',
      );
      assert.equal(lengths, 0);
    } finally {
      owner.destroy();
    }
  }));

for (const revoked of [true, false])
  test(`Scene carrier reveal ${revoked ? 'settles revoked native publication' : 'preserves still-current phase invariant'}`, () =>
    usingAtomicPaintFixture((f) => {
      const root = f.roots[0]!,
        chips = [atomicScenePaintSource(f, 0)];
      root.style.setProperty('opacity', '0.65', 'important');
      const owner = ReorderMotionPeerCarriers.prepare(
        f.view,
        f.f.line as unknown as HTMLElement,
        chips,
        1,
        1,
        () => true,
      );
      assert.ok(owner);
      const paint = new ReorderScenePaint(chips, owner);
      try {
        if (revoked) {
          owner.configure('create', () => true);
          assert.equal(
            owner.read(chips, () => true, admitReorderTargetLayout),
            'ready',
          );
          let current = true;
          const write = root.style.setProperty.bind(root.style);
          root.style.setProperty = (name, value, priority) => {
            write(name, value, priority);
            if (name === 'opacity' && value === '0') current = false;
          };
          assert.doesNotThrow(() => paint.reveal(() => current));
          assert.equal(current, false);
          assert.equal(root.style.getPropertyValue('opacity'), '0.65');
          assert.equal(root.style.getPropertyPriority('opacity'), 'important');
          assert.equal(root.getAttribute('data-reorder-motion-flow'), null);
          assert.equal(
            f.f.line.querySelector('.sp-reorder-motion-peer')!.style.getPropertyValue('opacity'),
            '0',
          );
        } else {
          assert.throws(() => paint.reveal(() => true), {
            message: 'reorder-peer-carrier-reveal-unavailable',
          });
        }
      } finally {
        f.causes.push(...paint.close(() => true));
      }
    }));
