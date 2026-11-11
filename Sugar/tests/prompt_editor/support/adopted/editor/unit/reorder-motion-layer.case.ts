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
/** Supported ViewPlugin measurements own native clip publication; controlled DOM cannot certify compositor pixels or scroll layout. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { undoDepth, redoDepth } from '@codemirror/commands';
import { reorderPreview } from '../../../../../../frontend/sugarprompt/editor/src/reorder-preview.ts';
import { readExactSource } from '../../../../../../frontend/sugarprompt/editor/src/exact-source.ts';
import {
  attachReorderMotionStage,
  reorderMotionAttachmentReady,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-layer.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { ReorderMotionPreparation } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-preparation.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

/** Existing Stage calibration is measured only after its initially empty native clip. */
function stageNode(f: ReturnType<typeof motionPaintFixture>): HTMLElement {
  const node = f.document.createElement('span');
  node.className = 'sp-reorder-motion-stage';
  node.style.width = '200px';
  node.style.height = '256px';
  return node as unknown as HTMLElement;
}

test('cold native preparation observes capture, drawn-layer wait, placement and ready; warm preparation reuses that exact Stage', () => {
  const f = motionPresentationFixture(),
    state = f.state,
    source = readExactSource(state);
  try {
    f.prepare();
    const capture = f.jobs.shift();
    assert.ok(capture);
    capture();
    assert.equal(f.lastRead, 'capture');
    assert.equal(f.view.dom.querySelector('.sp-reorder-motion-stage'), null);
    f.microtasks();
    const stage = f.view.dom.querySelector<HTMLElement>('.sp-reorder-motion-stage');
    assert.ok(stage);
    assert.equal(stage.style.visibility, 'hidden');
    assert.equal(f.paint.layerMount.style.width, '0px');
    assert.equal(f.state.field(reorderPreview).mask, null);

    const layer = f.jobs.shift();
    assert.ok(layer);
    layer();
    assert.equal(f.lastRead, 'layer');
    assert.equal(f.paint.layerMount.style.width, '240px');
    f.microtasks();
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(f.view.dom.querySelector('.sp-reorder-motion-stage'), stage);

    const placement = f.jobs.shift();
    assert.ok(placement);
    placement();
    assert.equal(f.lastRead, 'placement');
    f.microtasks();
    assert.equal(f.state.field(reorderPreview).mask, null);
    assert.equal(f.view.dom.querySelector('.sp-reorder-motion-stage'), stage);

    const ready = f.jobs.shift();
    assert.ok(ready);
    ready();
    assert.equal(f.lastRead, 'ready');
    f.microtasks();
    assert.ok(f.state.field(reorderPreview).mask);
    assert.equal(f.jobs.length, 0);
    const acceptedMask = f.state.field(reorderPreview).mask;

    f.publish();
    f.prepare();
    const warmCapture = f.jobs.shift();
    assert.ok(warmCapture);
    warmCapture();
    assert.equal(f.lastRead, 'capture');
    f.microtasks();
    assert.equal(f.state.field(reorderPreview).mask, acceptedMask);
    assert.equal(f.view.dom.querySelector('.sp-reorder-motion-stage'), stage);
    const warmReady = f.jobs.shift();
    assert.ok(warmReady);
    warmReady();
    assert.equal(f.lastRead, 'ready');
    f.microtasks();
    assert.ok(f.state.field(reorderPreview).mask);
    assert.notEqual(f.state.field(reorderPreview).mask, acceptedMask);
    assert.equal(f.view.dom.querySelector('.sp-reorder-motion-stage'), stage);
    assert.equal(f.jobs.length, 0);
    assert.equal(f.state.doc, state.doc);
    assert.equal(f.state.selection, state.selection);
    assert.equal(readExactSource(f.state), source);
    assert.deepEqual([undoDepth(f.state), redoDepth(f.state)], [0, 0]);
  } finally {
    f.destroy();
  }
});

test('native clip requires draw then exact original-extent READ and retains actual Stage identity', () => {
  const f = motionPaintFixture(),
    node = stageNode(f);
  Object.assign(f.view, { dispatch: () => {} });
  const lease = attachReorderMotionStage(f.view, node, 200, () => {});
  assert.ok(lease);
  try {
    assert.equal(lease.current(), true);
    assert.equal(f.layerMount.style.width, '0px');
    assert.equal(f.layerMount.style.overflow, 'clip');
    assert.equal(lease.read(), 'waiting');
    f.drawLayer();
    assert.equal(lease.read(), 'ready');
    assert.equal(node.parentElement, f.layerMount);
    assert.equal(f.layerMount.style.width, '240px');
    const marker = f.layerMount;
    f.drawLayer();
    assert.equal(f.layerMount, marker);
    assert.equal(node.parentElement, marker);
    f.scroller.scrollHeight++;
    assert.equal(lease.read(), 'unavailable');
    lease.release();
    assert.equal(f.layerMount.style.width, '0px');
    assert.equal(f.layerMount.style.height, '0px');
    assert.equal(f.layerMount.style.transform, 'none');
    assert.equal(lease.current(), false);
  } finally {
    node.remove();
    f.destroyLayer();
  }
});

for (const [left, top] of [
  [4.25, 12.5],
  [13.375, 7.8125],
] as const)
  test(`native clip WRITE preserves exact fractional READ coordinates ${left}/${top}`, () => {
    const f = motionPaintFixture(),
      node = stageNode(f),
      rectangle = f.scroller.getBoundingClientRect.bind(f.scroller),
      style = f.document.defaultView.getComputedStyle,
      rediscovery = new Error('controlled-marker-write-rediscovery');
    Object.assign(f.scroller.box, { left, top });
    const lease = attachReorderMotionStage(f.view, node, 200, () => {});
    assert.ok(lease);
    const phases: string[] = [];
    try {
      f.view.requestMeasure({
        read: () => {
          phases.push('read');
          assert.equal(lease.read(), 'waiting');
          assert.equal(f.layerMount.style.transform, 'none');
          assert.equal(f.layerMount.style.width, '0px');
          f.scroller.getBoundingClientRect = () => {
            throw rediscovery;
          };
          f.document.defaultView.getComputedStyle = () => {
            throw rediscovery;
          };
        },
        write: () => {
          phases.push('write');
          assert.equal(f.layerMount.style.left, '0px');
          assert.equal(f.layerMount.style.top, '0px');
          assert.equal(f.layerMount.style.transform, 'translate(0px,0px)');
          assert.equal(f.layerMount.style.width, '240px');
          assert.equal(f.layerMount.style.height, '100px');
          assert.equal(f.layerMount.style.overflow, 'visible');
          assert.equal(node.parentElement, f.layerMount);
        },
      });
      const frame = f.jobs.shift();
      assert.ok(frame);
      frame();
      assert.deepEqual(phases, ['read', 'write']);
      f.scroller.getBoundingClientRect = rectangle;
      f.document.defaultView.getComputedStyle = style;
      assert.equal(lease.read(), 'ready');
      assert.deepEqual(
        [f.layerMount.getBoundingClientRect().left, f.layerMount.getBoundingClientRect().top],
        [left, top],
      );
      lease.release();
      assert.equal(lease.current(), false);
      assert.equal(f.layerMount.style.transform, 'none');
      assert.equal(f.layerMount.style.width, '0px');
      assert.equal(f.layerMount.style.height, '0px');
      assert.deepEqual([f.scroller.box.left, f.scroller.box.top], [left, top]);
      assert.equal(f.jobs.length, 0);
    } finally {
      f.scroller.getBoundingClientRect = rectangle;
      f.document.defaultView.getComputedStyle = style;
      lease.release();
      node.remove();
      f.destroyLayer();
    }
  });

test('old lease retirement cannot empty a newer exact native clip issue', () => {
  const f = motionPaintFixture(),
    first = stageNode(f),
    second = stageNode(f);
  Object.assign(f.scroller.box, { left: 4.25, top: 12.5 });
  const old = attachReorderMotionStage(f.view, first, 200, () => {});
  assert.ok(old);
  try {
    assert.equal(old.read(), 'waiting');
    assert.equal(f.layerMount.style.width, '0px');
    const initialFrame = f.jobs.shift();
    assert.ok(initialFrame);
    initialFrame();
    assert.equal(old.read(), 'ready');
    assert.equal(f.layerMount.style.transform, 'translate(0px,0px)');
    old.release();
    assert.equal(f.layerMount.style.transform, 'none');
    first.remove();
    Object.assign(f.scroller.box, { left: 31.375, top: 17.25 });
    const current = attachReorderMotionStage(f.view, second, 200, () => {});
    assert.ok(current);
    assert.equal(current.read(), 'waiting');
    assert.equal(f.layerMount.style.width, '0px');
    const replacementFrame = f.jobs.shift();
    assert.ok(replacementFrame);
    replacementFrame();
    assert.equal(current.read(), 'ready');
    const marker = f.layerMount;
    assert.equal(marker.style.transform, 'translate(0px,0px)');
    old.release();
    assert.equal(current.current(), true);
    assert.equal(f.layerMount.style.width, '240px');
    assert.equal(f.layerMount, marker);
    assert.equal(second.parentElement, marker);
    assert.equal(marker.style.transform, 'translate(0px,0px)');
    current.release();
    assert.equal(marker.style.transform, 'none');
    assert.equal(marker.style.width, '0px');
    assert.equal(marker.style.height, '0px');
  } finally {
    first.remove();
    second.remove();
    f.destroyLayer();
  }
});

test('native layer removal clears registration and still removes its root after raw Stage retirement failure', () => {
  const f = motionPaintFixture(),
    node = stageNode(f),
    replacement = stageNode(f),
    failure = new Error('controlled-stage-retirement');
  let retired = 0;
  Object.assign(f.scroller.box, { left: 4.25, top: 12.5 });
  assert.ok(
    attachReorderMotionStage(f.view, node, 200, () => {
      retired++;
      assert.equal(reorderMotionAttachmentReady(f.view), false);
      assert.equal(
        attachReorderMotionStage(f.view, replacement, 200, () => {}),
        null,
      );
      throw failure;
    }),
  );
  const frame = f.jobs.shift();
  assert.ok(frame);
  frame();
  assert.equal(f.layerMount.style.transform, 'translate(0px,0px)');
  assert.throws(
    () => f.destroyLayer(),
    (cause) => cause === failure,
  );
  assert.equal(reorderMotionAttachmentReady(f.view), false);
  assert.equal(retired, 1);
  assert.equal(f.layerRoot.isConnected, false);
  assert.equal(
    attachReorderMotionStage(f.view, node, 200, () => {}),
    null,
  );
  assert.equal(f.jobs.length, 0);
  replacement.remove();
});

test('failed Stage construction releases the exact borrowed marker so the next genuine Stage can mount', () => {
  const f = motionPresentationFixture();
  let stage: ReorderPassiveStage | null = null;
  try {
    const prepared = preparePassiveMotion(f.view, 0);
    assert.ok(prepared);
    assert.throws(
      () =>
        new ReorderPassiveStage(f.view, {
          ...prepared,
          context: { ...prepared.context, appearanceCurrent: () => false },
        }),
      /Passive stage retired during creation/u,
    );
    stage = new ReorderPassiveStage(f.view, prepared);
    assert.equal(stage.current(), true);
    assert.ok(stage.mount(prepared));
  } finally {
    stage?.destroy();
    f.destroy();
  }
});

test('refused layer extent admission retires the complete exact Stage before unavailable publication and permits a fresh Stage', () => {
  const f = motionPresentationFixture();
  try {
    f.prepare();
    const first = f.jobs.shift();
    assert.ok(first);
    first();
    f.microtasks();
    const old = f.view.dom.querySelector('.sp-reorder-motion-stage');
    assert.ok(old);
    assert.equal(f.paint.layerMount.style.width, '0px');
    const nativeLayerFrame = f.jobs.shift();
    assert.ok(nativeLayerFrame);
    nativeLayerFrame();
    f.microtasks();
    assert.equal(f.view.dom.querySelector('.sp-reorder-motion-stage'), old);
    assert.equal(f.paint.layerMount.style.width, '240px');
    f.paint.scroller.scrollHeight++;
    f.deliver();
    assert.equal(old.isConnected, false);
    assert.equal(f.paint.layerMount.style.width, '0px');
    assert.equal(f.paint.layerMount.style.height, '0px');
    assert.equal(f.view.dom.querySelector('.sp-reorder-motion-stage'), null);
    f.paint.scroller.scrollHeight--;
    f.publish();
    f.prepare();
    f.deliver();
    const fresh = f.view.dom.querySelector('.sp-reorder-motion-stage');
    assert.ok(fresh);
    assert.notEqual(fresh, old);
    assert.equal(f.paint.layerMount.style.width, '240px');
  } finally {
    f.destroy();
  }
});

test('superseding preparation cannot calibrate beneath an undrawn native clip', () => {
  const f = motionPresentationFixture();
  let stage: ReorderPassiveStage | null = null;
  try {
    const first = preparePassiveMotion(f.view, 0);
    assert.ok(first);
    Object.assign(f.view, { dispatch: () => {} });
    stage = new ReorderPassiveStage(f.view, first);
    assert.equal(stage.readPlacement(first), null);
    f.publish();
    const superseding = preparePassiveMotion(f.view, 0);
    assert.ok(superseding);
    assert.notEqual(superseding.preview, first.preview);
    assert.equal(stage.readPlacement(superseding), null);
    assert.equal(stage.measured(), null);
    f.paint.drawLayer();
    assert.equal(stage.readPlacement(superseding)?.kind, 'placement');
  } finally {
    stage?.destroy();
    f.destroy();
  }
});

for (const blocked of ['line-background', 'line-generated'] as const)
  test(`below-text layer refuses actual ${blocked} before any coverage`, () => {
    const f = motionPaintFixture(),
      node = stageNode(f),
      read = f.document.defaultView.getComputedStyle.bind(f.document.defaultView);
    f.document.defaultView.getComputedStyle = (target, pseudo) => {
      const style = read(target, pseudo);
      if (target !== f.line) return style;
      if (blocked === 'line-background' && pseudo === undefined)
        return { ...style, backgroundColor: 'rgb(255, 255, 255)' };
      if (blocked === 'line-generated' && pseudo === '::before') return { ...style, content: '""' };
      return style;
    };
    const lease = attachReorderMotionStage(f.view, node, 200, () => {});
    assert.ok(lease);
    try {
      assert.equal(lease.read(), 'waiting');
      assert.equal(f.layerMount.style.width, '0px');
      assert.equal(f.layerMount.style.height, '0px');
      lease.release();
    } finally {
      node.remove();
      f.destroyLayer();
    }
  });

test('refused admission publishes fallback before exact Stage cleanup and unavailable release', () => {
  const f = motionPresentationFixture(),
    order: string[] = [];
  const preparation = new ReorderMotionPreparation(f.view, {
    current: () => true,
    rects: () => f.origin,
    refresh: () => {},
    observe: (action) => action(),
    schedule: (request) => f.view.requestMeasure(request),
  });
  try {
    preparation.prepare(f.input, {
      current: () => true,
      select: () => [],
      pending: () => {},
      replaceContext: () => {},
      fallback: () => {
        order.push('fallback');
        assert.ok(f.view.dom.querySelector('.sp-reorder-motion-stage'));
      },
      unavailable: () => {
        order.push('unavailable');
        assert.equal(f.view.dom.querySelector('.sp-reorder-motion-stage'), null);
        assert.equal(f.paint.layerMount.style.width, '0px');
      },
      ready: () => assert.fail('refused-extent-must-not-publish'),
    });
    const first = f.jobs.shift();
    assert.ok(first);
    first();
    f.microtasks();
    assert.equal(f.paint.layerMount.style.width, '0px');
    const nativeLayerFrame = f.jobs.shift();
    assert.ok(nativeLayerFrame);
    nativeLayerFrame();
    f.microtasks();
    assert.equal(f.paint.layerMount.style.width, '240px');
    f.paint.scroller.scrollHeight++;
    f.deliver();
    assert.deepEqual(order, ['fallback', 'unavailable']);
  } finally {
    preparation.cancel();
    f.destroy();
  }
});

for (const [property, value] of [
  ['opacity', '0.5'],
  ['filter', 'blur(1px)'],
  ['backdrop-filter', 'blur(1px)'],
  ['background-color', 'rgb(255, 255, 255)'],
  ['background-image', 'linear-gradient(rgb(255, 255, 255), rgb(255, 255, 255))'],
  ['background-blend-mode', 'multiply'],
  ['box-shadow', 'rgb(0, 0, 0) 0px 0px 1px 0px'],
  ['mix-blend-mode', 'multiply'],
  ['isolation', 'isolate'],
  ['clip-path', 'inset(1px)'],
  ['outline-style', 'solid'],
  ['visibility', 'hidden'],
  ['text-decoration-line', 'underline'],
  ['text-shadow', 'rgb(0, 0, 0) 1px 1px 1px'],
] as const)
  test(`shared native scroller ${property} refuses passive copied effects before paint`, () => {
    const f = motionPaintFixture(),
      node = stageNode(f),
      read = f.document.defaultView.getComputedStyle.bind(f.document.defaultView);
    f.document.defaultView.getComputedStyle = (target, pseudo) => {
      const style = read(target, pseudo);
      if (target !== f.scroller || pseudo !== undefined) return style;
      return {
        ...style,
        backgroundColor: property === 'background-color' ? value : 'rgba(0, 0, 0, 0)',
        getPropertyValue: (name: string) =>
          name === property ? value : style.getPropertyValue(name),
      };
    };
    const lease = attachReorderMotionStage(f.view, node, 200, () => {});
    assert.ok(lease);
    try {
      assert.equal(lease.read(), 'waiting');
      assert.equal(f.layerMount.style.width, '0px');
      assert.equal(f.layerMount.style.height, '0px');
      lease.release();
    } finally {
      node.remove();
      f.destroyLayer();
    }
  });
