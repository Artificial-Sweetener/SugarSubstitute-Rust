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
/** The native viewport owner admits actual clip semantics and fences every later publication with its exact declaration. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  attachReorderMotionStage,
  ReorderMotionAttachment,
  reorderMotionAttachmentReady,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-layer.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';
import { wholeFixture } from './reorder-pointer-stage-fixture.ts';

/** The same real layer transport attaches one externally calibrated Stage under its existing finite lease. */
function pendingClippingFixture() {
  const f = motionPaintFixture(),
    node = f.document.createElement('span');
  node.className = 'sp-reorder-motion-stage';
  node.style.width = '200px';
  node.style.height = '256px';
  Object.assign(f.view, { dispatch: () => {} });
  const lease = attachReorderMotionStage(f.view, node as unknown as HTMLElement, 200, () => {});
  assert.ok(lease);
  return { f, node, lease };
}

/** Existing admission controls start after the actual native clip batch. */
function clippingFixture() {
  const fixture = pendingClippingFixture();
  fixture.f.drawLayer();
  return fixture;
}

test('an issued whole scene delegates active clipping to its admitted native viewport', () => {
  const f = wholeFixture();
  try {
    f.prepare();
    f.deliver();
    const scene = f.paint.editor.querySelector('.sp-reorder-motion-paint');
    assert.ok(scene);
    assert.equal(f.presentation.displayPaint(f.input)?.current(), true);
    assert.equal(scene.style.clip, 'auto');
    assert.equal(f.paint.layerMount.style.overflow, 'visible');
    assert.equal(f.paint.layerMount.style.getPropertyPriority('overflow'), 'important');
  } finally {
    f.destroy();
  }
});

for (const [name, value] of [
  ['overflow-x', 'clip'],
  ['overflow-y', 'hidden'],
  ['overflow-clip-margin', '8px'],
  ['border-top-width', '1px'],
  ['padding-right', '1px'],
] as const) {
  test(`actual unsupported computed ${name} refuses unchanged viewport geometry`, () => {
    const { f, node, lease } = clippingFixture();
    const native = f.document.defaultView.getComputedStyle.bind(f.document.defaultView);
    try {
      f.document.defaultView.getComputedStyle = (element, pseudo) => {
        const style = native(element, pseudo);
        if (!Object.is(element, f.layerMount)) return style;
        return {
          ...style,
          getPropertyValue: (property: string) =>
            property === name ? value : style.getPropertyValue(property),
        };
      };
      assert.equal(lease.read(), 'unavailable');
      assert.equal(lease.current(), true);
    } finally {
      lease.release();
      node.remove();
      f.destroyLayer();
    }
  });
}

test('owned critical declarations protect author priority and a direct mutation cannot borrow ready clipping', () => {
  const { f, node, lease } = clippingFixture();
  try {
    assert.equal(lease.read(), 'ready');
    for (const name of [
      'position',
      'overflow',
      'overflow-clip-margin',
      'margin',
      'padding',
      'border',
      'box-sizing',
      'left',
      'top',
      'width',
      'height',
      'transform',
    ])
      assert.equal(f.layerMount.style.getPropertyPriority(name), 'important');
    const reads = f.styleReads;
    assert.equal(lease.current(), true);
    assert.equal(f.styleReads, reads);
    f.layerMount.style.setProperty('overflow', 'clip', 'important');
    assert.equal(lease.current(), false);
    assert.equal(lease.read(), 'unavailable');
    assert.equal(f.styleReads, reads);
    lease.release();
    assert.equal(f.layerMount.style.width, '0px');
    assert.equal(f.layerMount.style.height, '0px');
    assert.equal(f.layerMount.style.overflow, 'clip');
    assert.equal(f.layerMount.style.getPropertyPriority('width'), 'important');
    assert.equal(lease.current(), false);
  } finally {
    lease.release();
    node.remove();
    f.destroyLayer();
  }
});

test('computed clip admission cannot publish after synchronous native style reentry changes the owned declaration', () => {
  const { f, node, lease } = clippingFixture();
  const native = f.document.defaultView.getComputedStyle.bind(f.document.defaultView);
  let reentered = false;
  try {
    f.document.defaultView.getComputedStyle = (element, pseudo) => {
      const style = native(element, pseudo);
      if (Object.is(element, f.layerMount)) {
        reentered = true;
        f.layerMount.style.setProperty('overflow-clip-margin', '8px', 'important');
      }
      return style;
    };
    assert.equal(lease.read(), 'unavailable');
    assert.equal(reentered, true);
    assert.equal(lease.current(), false);
  } finally {
    lease.release();
    node.remove();
    f.destroyLayer();
  }
});

/** Retirement proves actual physical suppression, not just a rejected lease. */
function assertEmpty(node: HTMLElement): void {
  assert.equal(node.style.width, '0px');
  assert.equal(node.style.height, '0px');
  assert.equal(node.style.transform, 'none');
  assert.equal(node.style.overflow, 'clip');
  for (const name of ['width', 'height', 'transform', 'overflow'])
    assert.equal(node.style.getPropertyPriority(name), 'important');
}

test('a resumed old setter is emptied before the same native clip can admit another Stage', () => {
  const { f, node, lease } = pendingClippingFixture();
  const style = f.layerMount.style,
    native = style.setProperty.bind(style),
    next = f.document.createElement('span');
  next.className = 'sp-reorder-motion-stage';
  next.style.width = '200px';
  next.style.height = '256px';
  let reentered = false;
  try {
    style.setProperty = (name, value, priority) => {
      if (!reentered && name === 'width' && value !== '0px') {
        reentered = true;
        lease.release();
        assertEmpty(f.layerMount);
        assert.equal(reorderMotionAttachmentReady(f.view), false);
        assert.equal(
          attachReorderMotionStage(f.view, next as unknown as HTMLElement, 200, () => {}),
          null,
        );
      }
      native(name, value, priority);
    };
    f.drawLayer();
    assert.equal(reentered, true);
    assertEmpty(f.layerMount);
    assert.equal(lease.current(), false);
    assert.equal(lease.read(), 'unavailable');
    style.setProperty = native;
    const newer = attachReorderMotionStage(f.view, next as unknown as HTMLElement, 200, () => {});
    assert.ok(newer);
    f.drawLayer();
    assert.equal(newer.read(), 'ready');
    const accepted = style.cssText;
    lease.release();
    assert.equal(style.cssText, accepted);
    assert.equal(newer.read(), 'ready');
    newer.release();
    assertEmpty(f.layerMount);
  } finally {
    style.setProperty = native;
    lease.release();
    node.remove();
    next.remove();
    f.destroyLayer();
  }
});

for (const frontier of ['current', 'stamp'] as const) {
  test(`native declaration ${frontier} retirement cannot certify a stale physical declaration`, () => {
    const { f, node, lease } =
      frontier === 'current' ? clippingFixture() : pendingClippingFixture();
    const style = f.layerMount.style,
      descriptor = Object.getOwnPropertyDescriptor(style, 'cssText');
    assert.ok(descriptor?.get);
    const read = descriptor.get;
    const attribute = f.layerMount.getAttribute.bind(f.layerMount);
    const native = style.setProperty.bind(style);
    let reentered = false,
      armed = frontier === 'current';
    try {
      style.setProperty = (name, value, priority) => {
        native(name, value, priority);
        if (name === 'height' && value !== '0px') armed = true;
      };
      /** Warm admission consumes native reflection; cold stamping still consumes canonical CSSOM. */
      const retire = (): void => {
        if (armed && !reentered) {
          reentered = true;
          lease.release();
        }
      };
      f.layerMount.getAttribute = (name) => {
        const before = attribute(name);
        if (frontier === 'current' && name === 'style') retire();
        return before;
      };
      Object.defineProperty(style, 'cssText', {
        ...descriptor,
        get: () => {
          const before = Reflect.apply(read, style, []) as string;
          if (frontier === 'stamp') retire();
          return before;
        },
      });
      if (frontier === 'current') assert.equal(lease.current(), false);
      else f.drawLayer();
      assert.equal(reentered, true);
      assert.equal(lease.read(), 'unavailable');
      assertEmpty(f.layerMount);
      Object.defineProperty(style, 'cssText', descriptor);
      f.layerMount.getAttribute = attribute;
      assert.equal(reorderMotionAttachmentReady(f.view), true);
    } finally {
      Object.defineProperty(style, 'cssText', descriptor);
      f.layerMount.getAttribute = attribute;
      style.setProperty = native;
      lease.release();
      node.remove();
      f.destroyLayer();
    }
  });
}

test('a raw marker write failure preserves every ordered reset cause and leaves admission closed', () => {
  const { f, node, lease } = pendingClippingFixture();
  const style = f.layerMount.style,
    native = style.setProperty.bind(style),
    primary = {};
  const resets: string[] = [];
  let failed = false;
  try {
    style.setProperty = (name, value, priority) => {
      if (!failed && name === 'height' && value !== '0px') {
        failed = true;
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw primary;
      }
      native(name, value, priority);
      if (failed) {
        resets.push(name);
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (name === 'width') throw null;
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        if (name === 'height') throw undefined;
      }
    };
    assert.throws(
      () => f.drawLayer(),
      (cause: unknown) => {
        assert.ok(cause instanceof AggregateError);
        assert.deepEqual(cause.errors, [primary, null, undefined]);
        return true;
      },
    );
    assert.deepEqual(resets, [
      'position',
      'overflow',
      'overflow-clip-margin',
      'margin',
      'padding',
      'border',
      'box-sizing',
      'left',
      'top',
      'transform',
      'width',
      'height',
    ]);
    assertEmpty(f.layerMount);
    assert.equal(reorderMotionAttachmentReady(f.view), false);
    assert.equal(lease.read(), 'unavailable');
  } finally {
    style.setProperty = native;
    lease.release();
    node.remove();
    f.destroyLayer();
  }
});

test('late captured cleanup cannot change a fresh native owner installed during destruction', () => {
  const { f, node, lease } = pendingClippingFixture();
  const old = f.layerMount,
    style = old.style,
    native = style.setProperty.bind(style);
  const fresh: {
    node: HTMLElement | null;
    owner: ReorderMotionAttachment | null;
  } = { node: null, owner: null };
  let accepted = '',
    reentered = false;
  try {
    style.setProperty = (name, value, priority) => {
      if (!reentered && name === 'width' && value !== '0px') {
        reentered = true;
        f.destroyLayer();
        fresh.owner = new ReorderMotionAttachment(f.view);
        fresh.node = f.scroller.querySelector(
          '.sp-reorder-motion-mount',
        ) as unknown as HTMLElement | null;
        assert.ok(fresh.node);
        accepted = fresh.node.style.cssText;
      }
      native(name, value, priority);
    };
    f.drawLayer();
    assert.equal(reentered, true);
    assertEmpty(old);
    assert.ok(fresh.node);
    assert.equal(fresh.node.style.cssText, accepted);
    assert.equal(reorderMotionAttachmentReady(f.view), true);
    lease.release();
    assert.equal(fresh.node.style.cssText, accepted);
    assert.equal(lease.read(), 'unavailable');
  } finally {
    style.setProperty = native;
    node.remove();
    fresh.owner?.destroy();
  }
});
test('a declaration getter cannot retain current clipping after changing exact marker ancestry', () => {
  const { f, node, lease } = clippingFixture();
  const style = f.layerMount.style,
    descriptor = Object.getOwnPropertyDescriptor(style, 'cssText');
  assert.ok(descriptor?.get);

  const attribute = f.layerMount.getAttribute.bind(f.layerMount);
  let moved = false;
  try {
    f.layerMount.getAttribute = (name) => {
      const before = attribute(name);
      if (!moved) {
        moved = true;
        f.layerMount.remove();
      }
      return before;
    };
    assert.equal(lease.current(), false);
    assert.equal(lease.read(), 'unavailable');
    assert.equal(moved, true);
    Object.defineProperty(style, 'cssText', descriptor);
    f.layerMount.getAttribute = attribute;
    f.layerRoot.append(f.layerMount as unknown as typeof node);
    assert.equal(lease.read(), 'ready');
    lease.release();
    assertEmpty(f.layerMount);
  } finally {
    Object.defineProperty(style, 'cssText', descriptor);
    f.layerMount.getAttribute = attribute;
    lease.release();
    node.remove();
    f.destroyLayer();
  }
});
