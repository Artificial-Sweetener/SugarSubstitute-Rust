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
/** Actual attachment and Stage owners consume native ancestor geometry without an inverse CM Layer scale. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  attachReorderMotionStage,
  ReorderMotionAttachment,
  reorderMotionAttachmentReady,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-layer.ts';
import { ReorderPassiveStage } from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-stage.ts';
import { preparePassiveMotion } from '../../../../../../frontend/sugarprompt/editor/src/reorder-passive-context.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
import { motionPresentationFixture } from './reorder-motion-presentation-fixture.ts';

/** Literal viewport-CSS-pixel rectangles supply independent physical operands, not a transform/layout implementation. */
function rectangle(left: number, top: number, width: number, height: number): DOMRect {
  return {
    left,
    top,
    width,
    height,
    right: left + width,
    bottom: top + height,
    x: left,
    y: top,
    toJSON: () => ({}),
  };
}

test('scroll attachment is a zero-flow inert below-text sibling with finite queued teardown', () => {
  const f = motionPaintFixture(),
    content = f.content.childNodes.slice(),
    extent = [
      f.scroller.scrollWidth,
      f.scroller.scrollHeight,
      f.scroller.clientWidth,
      f.scroller.clientHeight,
    ],
    root = f.layerRoot,
    mount = f.layerMount;
  const causes: unknown[] = [];
  try {
    assert.equal(root.parentElement, f.scroller);
    assert.equal(f.content.parentElement, f.scroller);
    assert.equal(root.className, 'sp-reorder-motion-layer');
    assert.equal(root.style.position, 'absolute');
    assert.equal(root.style.width, '0px');
    assert.equal(root.style.height, '0px');
    assert.equal(root.style.contain, 'layout size');
    assert.equal(root.style.transform, 'none');
    assert.equal(root.style.zIndex, '-1');
    assert.equal(root.style.getPropertyPriority('transform'), 'important');
    assert.equal(root.style.pointerEvents, 'none');
    assert.equal(Reflect.get(root, 'inert'), true);
    assert.equal(root.getAttribute('aria-hidden'), 'true');
    assert.equal(mount.style.overflow, 'clip');
    assert.deepEqual(f.content.childNodes, content);
    const node = f.document.createElement('span');
    node.className = 'sp-reorder-motion-stage';
    node.style.width = '200px';
    node.style.height = '256px';
    let retired = 0;
    const lease = attachReorderMotionStage(f.view, node as unknown as HTMLElement, 200, () => {
      retired++;
    });
    assert.ok(lease);
    assert.equal(lease.read(), 'waiting');
    const queued = f.jobs.shift();
    assert.ok(queued);
    f.attachment.destroy();
    assert.equal(retired, 1);
    assert.equal(root.isConnected, false);
    assert.equal(reorderMotionAttachmentReady(f.view), false);
    queued();
    assert.equal(lease.read(), 'unavailable');
    assert.deepEqual(f.content.childNodes, content);
    assert.deepEqual(
      [
        f.scroller.scrollWidth,
        f.scroller.scrollHeight,
        f.scroller.clientWidth,
        f.scroller.clientHeight,
      ],
      extent,
    );
    assert.equal(f.scroller.querySelector('.sp-reorder-motion-layer'), null);
  } catch (cause) {
    causes.push(cause);
  }
  try {
    f.destroyLayer();
  } catch (cause) {
    causes.push(cause);
  }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'attachment-lifetime-control-failed');
});

test('inherited native scale calibrates Stage without cancel-and-restore transforms', () => {
  const f = motionPresentationFixture();
  const causes: unknown[] = [];
  let stage: ReorderPassiveStage | null = null;
  try {
    const initial = preparePassiveMotion(f.view, 0);
    assert.ok(initial);
    const prepared = {
      ...initial,
      context: { ...initial.context, scaleX: 1.25, scaleY: 1.25 },
    };
    Object.assign(f.view, { scaleX: 1.25, scaleY: 1.25 });
    f.paint.scroller.getBoundingClientRect = () => rectangle(0, 0, 300, 125);
    f.paint.layerRoot.getBoundingClientRect = () => rectangle(0, 0, 0, 0);
    f.paint.layerMount.getBoundingClientRect = () => rectangle(0, 0, 300, 125);
    stage = new ReorderPassiveStage(f.view, prepared);
    stage.node.getBoundingClientRect = () => rectangle(0, 0, 250, 320);
    assert.equal(stage.readLayer(), 'waiting');
    f.paint.drawLayer();
    assert.equal(stage.readLayer(), 'ready');
    const read = stage.readPlacement(prepared);
    assert.ok(read);
    assert.deepEqual(read, {
      kind: 'placement',
      left: 0,
      top: 0,
      scaleX: 1.25,
      scaleY: 1.25,
    });
    stage.place(prepared, read);
    assert.equal(f.paint.layerRoot.style.transform, 'none');
    assert.equal(stage.node.style.transform, 'none');
    assert.equal(stage.node.style.getPropertyPriority('transform'), 'important');
    assert.deepEqual(stage.measured(), { scaleX: 1.25, scaleY: 1.25 });
    assert.equal(stage.current(), true);
    f.paint.layerRoot.style.transform = 'scale(0.8)';
    assert.equal(stage.current(), false);
  } catch (cause) {
    causes.push(cause);
  }
  for (const release of [() => stage?.destroy(), () => f.destroy()])
    try {
      release();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'attachment-inherited-scale-control-failed');
});

for (const paint of ['background', 'generated'] as const)
  test(
    'owned scroll attachment refuses actual unsupported ' +
      paint +
      ' paint before clipping admission',
    () => {
      const f = motionPaintFixture(),
        native = f.document.defaultView.getComputedStyle;
      const causes: unknown[] = [];
      let lease: ReturnType<typeof attachReorderMotionStage> = null;
      try {
        f.document.defaultView.getComputedStyle = (node, pseudo) => {
          const style = native(node, pseudo);
          if (node !== f.layerRoot) return style;
          if (paint === 'generated' && pseudo === '::before')
            return { ...style, content: '""', display: 'block', opacity: '1' };
          return paint === 'background' ? { ...style, backgroundColor: 'rgb(255,255,255)' } : style;
        };
        const node = f.document.createElement('span');
        node.className = 'sp-reorder-motion-stage';
        node.style.width = '200px';
        node.style.height = '256px';
        lease = attachReorderMotionStage(f.view, node as unknown as HTMLElement, 200, () => {});
        assert.ok(lease);
        f.drawLayer();
        assert.equal(lease.read(), 'waiting');
        assert.equal(f.layerMount.style.width, '0px');
        assert.equal(f.layerMount.style.height, '0px');
        assert.equal(node.parentElement, f.layerMount);
      } catch (cause) {
        causes.push(cause);
      }
      f.document.defaultView.getComputedStyle = native;
      for (const release of [() => lease?.release(), () => f.destroyLayer()])
        try {
          release();
        } catch (cause) {
          causes.push(cause);
        }
      if (causes.length === 1) throw causes[0];
      if (causes.length)
        throw new AggregateError(causes, 'attachment-paint-admission-control-failed');
    },
  );

test('detached native construction becomes ready on connection without replacing its plugin owner', () => {
  const f = motionPaintFixture(),
    causes: unknown[] = [];
  let owner: ReorderMotionAttachment | null = null;
  let lease: ReturnType<typeof attachReorderMotionStage> = null;
  try {
    f.destroyLayer();
    f.editor.remove();
    assert.equal(f.editor.isConnected, false);
    owner = new ReorderMotionAttachment(f.view);
    const root = f.scroller.querySelector('.sp-reorder-motion-layer');
    assert.ok(root);
    const clip = root.querySelector('.sp-reorder-motion-mount');
    assert.ok(clip);
    assert.equal(root.isConnected, false);
    assert.equal(root.parentElement, f.scroller);
    assert.equal(owner.current(), false);
    assert.equal(reorderMotionAttachmentReady(f.view), false);
    const stage = f.document.createElement('span');
    stage.className = 'sp-reorder-motion-stage';
    stage.style.width = '200px';
    stage.style.height = '256px';
    assert.equal(
      attachReorderMotionStage(f.view, stage as unknown as HTMLElement, 200, () => {}),
      null,
    );
    f.document.body!.append(f.editor);
    assert.equal(owner.current(), true);
    assert.equal(reorderMotionAttachmentReady(f.view), true);
    assert.equal(f.scroller.querySelector('.sp-reorder-motion-layer'), root);
    assert.equal(root.querySelector('.sp-reorder-motion-mount'), clip);
    lease = attachReorderMotionStage(f.view, stage as unknown as HTMLElement, 200, () => {});
    assert.ok(lease);
    assert.equal(lease.read(), 'waiting');
    const frame = f.jobs.shift();
    assert.ok(frame);
    frame();
    assert.equal(lease.read(), 'ready');
    assert.equal(stage.parentElement, clip);
    f.editor.remove();
    assert.equal(owner.current(), false);
    assert.equal(lease.current(), false);
    assert.equal(lease.read(), 'unavailable');
  } catch (cause) {
    causes.push(cause);
  }
  for (const release of [() => lease?.release(), () => owner?.destroy(), () => f.destroyLayer()])
    try {
      release();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length)
    throw new AggregateError(causes, 'attachment-detached-construction-control-failed');
});
