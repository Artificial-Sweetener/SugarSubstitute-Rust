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
/** The sole external feedback frame owns coalescing/currentness and every exact callback/paint release. */
import assert from 'node:assert/strict';
import test from 'node:test';
import { ReorderPointerFeedback } from '../../../../../../frontend/sugarprompt/editor/src/reorder-pointer-feedback.ts';
import { reorderGrabGeometry } from '../../../../../../frontend/sugarprompt/editor/src/reorder-grab-geometry.ts';
import { paintFixture } from './reorder-marker-fixture.ts';

/** Real feedback uses an independently specified native grab rectangle and the existing finite platform transport. */
function feedbackFixture() {
  const { f, nodes } = paintFixture();
  const feedback = new ReorderPointerFeedback(f.view);
  feedback.measureAppearance();
  const geometry = reorderGrabGeometry(
    [{ kind: 'chip', index: 0, left: 10, right: 30, top: 10, bottom: 30 }],
    0,
    { x: 14, y: 12 },
  );
  assert.ok(geometry);
  /** Adopt one fresh external paint lease without altering native editor source or geometry. */
  function begin() {
    const proxy = f.view.dom.ownerDocument.createElement('span');
    feedback.begin({ node: proxy, geometry: geometry! }, { x: 14, y: 12 });
    const surface = nodes.findLast(
      (node) => node.className.split(' ').includes('sp-reorder-drag-surface') && !node.removed,
    );
    assert.ok(surface);
    return { proxy, surface };
  }
  return { f, nodes, feedback, begin };
}

test('feedback coalesces only latest coordinates into one finite frame and creates no idle successor', () => {
  const c = feedbackFixture();
  try {
    const { surface } = c.begin();
    const action = (callback: () => void): void => callback();
    c.feedback.follow({ x: 40, y: 50 }, () => true, action);
    c.feedback.follow({ x: 70, y: 80 }, () => true, action);
    assert.equal(c.f.frames.size, 1);
    assert.equal(surface.style.transform, 'translate(10px, 10px)');
    c.f.frame(16);
    assert.equal(surface.style.transform, 'translate(66px, 78px)');
    assert.equal(surface.style.left, '0px');
    assert.equal(c.f.frames.size, 0);
    assert.equal(surface.style.top, '0px');
  } finally {
    c.feedback.close();
    c.f.destroy();
  }
});

test('fractional and negative translation preserves captured dimensions and probe hotspot without moving its zero anchors', () => {
  const c = feedbackFixture();
  try {
    const { surface } = c.begin();
    c.feedback.follow(
      { x: -2.5, y: 3.25 },
      () => true,
      (action) => action(),
    );
    c.f.frame(16);
    assert.equal(surface.style.transform, 'translate(-6.5px, 1.25px)');
    assert.equal(surface.style.left, '0px');
    assert.equal(surface.style.top, '0px');
    assert.equal(surface.style.width, '20px');
    assert.equal(surface.style.height, '20px');
    assert.deepEqual(c.feedback.probe({ x: -2.5, y: 3.25 }), {
      x: 3.5,
      y: 11.25,
    });
  } finally {
    c.feedback.close();
    c.f.destroy();
  }
});

test('initial translation reentry never appends the retired old surface after a new hold mounts', () => {
  const c = feedbackFixture(),
    document = c.f.view.dom.ownerDocument;
  const create = document.createElement.bind(document);
  let entered = false,
    old: HTMLElement | undefined;
  try {
    document.createElement = ((tag: string) => {
      const node = create(tag);
      let value = '';
      Object.defineProperty(node.style, 'transform', {
        configurable: true,
        get: () => value,
        set: (next: string) => {
          if (!entered && node.className.includes('sp-reorder-drag-surface')) {
            entered = true;
            old = node;
            c.begin();
          }
          value = next;
        },
      });
      return node;
    }) as typeof document.createElement;
    c.begin();
    assert.ok(old);
    assert.equal(Array.from(document.body.childNodes).includes(old), false);
    assert.equal(c.nodes.find((node) => (node as unknown) === old)?.removed, true);
    const current = c.nodes.findLast(
      (node) => node.className.includes('sp-reorder-drag-surface') && !node.removed,
    );
    assert.ok(current);
    assert.equal(
      Array.from(document.body.childNodes).some((node) => (node as unknown) === current),
      true,
    );
    assert.equal(current.style.transform, 'translate(10px, 10px)');
  } finally {
    document.createElement = create;
    c.feedback.close();
    c.f.destroy();
  }
});

test("translation setter reentry cannot overwrite the newer hold's painted-coordinate admission", () => {
  const c = feedbackFixture();
  try {
    const { surface } = c.begin();
    let newer: ReturnType<typeof c.begin> | undefined;
    Object.defineProperty(surface.style, 'transform', {
      configurable: true,
      set: () => {
        newer = c.begin();
      },
    });
    c.feedback.follow(
      { x: 40, y: 50 },
      () => true,
      (action) => action(),
    );
    c.f.frame(16);
    assert.ok(newer);
    assert.equal(newer.surface.style.transform, 'translate(10px, 10px)');
    c.feedback.follow(
      { x: 40, y: 50 },
      () => true,
      (action) => action(),
    );
    assert.equal(c.f.frames.size, 1);
    c.f.frame(32);
    assert.equal(newer.surface.style.transform, 'translate(36px, 48px)');
    assert.equal(surface.removed, true);
  } finally {
    c.feedback.close();
    c.f.destroy();
  }
});

for (const primary of [null, undefined]) {
  test(`translation setter preserves raw ${String(primary)} and retires its exact paint`, () => {
    const c = feedbackFixture();
    try {
      const { surface, proxy } = c.begin();
      Object.defineProperty(surface.style, 'transform', {
        configurable: true,
        set: () => {
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw primary;
        },
      });
      c.feedback.follow(
        { x: 40, y: 50 },
        () => true,
        (action) => action(),
      );
      const caught: unknown[] = [];
      try {
        c.f.frame(16);
      } catch (cause) {
        caught.push(cause);
      }
      assert.equal(caught.length, 1);
      assert.equal(caught[0], primary);
      assert.equal(surface.removed, true);
      assert.equal(c.nodes.find((node) => (node as unknown) === proxy)?.removed, true);
      assert.equal(c.f.frames.size, 0);
    } finally {
      c.feedback.close();
      c.f.destroy();
    }
  });
}

test('close and new holds fence retained old callbacks before any paint write', () => {
  const c = feedbackFixture();
  try {
    const first = c.begin();
    c.feedback.follow(
      { x: 40, y: 50 },
      () => true,
      (callback) => callback(),
    );
    const stale = [...c.f.frames.values()][0]!;
    c.feedback.close();
    const second = c.begin();
    stale(16);
    assert.equal(first.surface.removed, true);
    assert.equal(second.surface.style.transform, 'translate(10px, 10px)');
    assert.equal(second.surface.style.top, '0px');
    assert.equal(c.f.frames.size, 0);
  } finally {
    c.feedback.close();
    c.f.destroy();
  }
});

test('rejected currentness retires the single callback without inspecting or changing paint', () => {
  const c = feedbackFixture();
  try {
    const { surface } = c.begin();
    c.feedback.follow(
      { x: 40, y: 50 },
      () => false,
      (callback) => callback(),
    );
    c.f.frame(16);
    assert.equal(surface.style.transform, 'translate(10px, 10px)');
    assert.equal(surface.style.top, '0px');
    assert.equal(c.f.frames.size, 0);
  } finally {
    c.feedback.close();
    c.f.destroy();
  }
});

test('close independently releases the exact pending callback and both paint leases after all three failures', () => {
  const c = feedbackFixture(),
    clock = new Error('clock-release'),
    surfaceCause = new Error('surface-release'),
    proxyCause = new Error('proxy-release');
  const window = c.f.view.dom.ownerDocument.defaultView!;
  const cancel = window.cancelAnimationFrame;
  try {
    const { proxy, surface } = c.begin();
    c.feedback.follow(
      { x: 40, y: 50 },
      () => true,
      (callback) => callback(),
    );
    const issued = [...c.f.frames.keys()][0]!;
    let released: number | undefined;
    window.cancelAnimationFrame = (handle) => {
      released = handle;
      cancel.call(window, handle);
      throw clock;
    };
    const removeSurface = surface.remove,
      removeProxy = proxy.remove.bind(proxy);
    surface.remove = () => {
      removeSurface.call(surface);
      throw surfaceCause;
    };
    proxy.remove = () => {
      removeProxy();
      throw proxyCause;
    };
    assert.throws(
      () => c.feedback.close(),
      (error) =>
        error instanceof AggregateError &&
        error.errors.length === 3 &&
        error.errors[0] === clock &&
        error.errors[1] === surfaceCause &&
        error.errors[2] === proxyCause,
    );
    assert.equal(released, issued);
    assert.equal(surface.removed, true);
    assert.equal(c.f.frames.size, 0);
    c.feedback.close();
  } finally {
    window.cancelAnimationFrame = cancel;
    c.feedback.close();
    c.f.destroy();
  }
});

for (const primary of [null, undefined]) {
  test(`frame preserves thrown ${String(primary)} before every removal failure`, () => {
    const c = feedbackFixture(),
      surfaceCause = new Error('surface-release'),
      proxyCause = new Error('proxy-release');
    try {
      const { proxy, surface } = c.begin();
      const removeSurface = surface.remove,
        removeProxy = proxy.remove.bind(proxy);
      surface.remove = () => {
        removeSurface.call(surface);
        throw surfaceCause;
      };
      proxy.remove = () => {
        removeProxy();
        throw proxyCause;
      };
      c.feedback.follow(
        { x: 40, y: 50 },
        () => {
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          throw primary;
        },
        (callback) => callback(),
      );
      assert.throws(
        () => c.f.frame(16),
        (error) =>
          error instanceof AggregateError &&
          error.errors[0] === primary &&
          error.errors[1] instanceof AggregateError &&
          error.errors[1].errors[0] === surfaceCause &&
          error.errors[1].errors[1] === proxyCause,
      );
      assert.equal(surface.removed, true);
      assert.equal(c.f.frames.size, 0);
    } finally {
      c.feedback.close();
      c.f.destroy();
    }
  });
}

test('currentness reentry installing a newer hold cannot paint old coordinates into its new surface', () => {
  const c = feedbackFixture();
  try {
    c.begin();
    let newer: ReturnType<typeof c.begin> | undefined;
    c.feedback.follow(
      { x: 40, y: 50 },
      () => {
        newer = c.begin();
        return true;
      },
      (callback) => callback(),
    );
    c.f.frame(16);
    assert.ok(newer);
    assert.equal(newer.surface.style.transform, 'translate(10px, 10px)');
    assert.equal(newer.surface.style.top, '0px');
    assert.equal(c.f.frames.size, 0);
  } finally {
    c.feedback.close();
    c.f.destroy();
  }
});

test('prepared pointer frame writes cached style without DOM/style/geometry rediscovery', () => {
  const c = feedbackFixture();
  const { surface } = c.begin(),
    style = surface.style;
  const window = c.f.view.dom.ownerDocument.defaultView!;
  const computed = window.getComputedStyle;
  const surfaceStyle = Object.getOwnPropertyDescriptor(surface, 'style');
  assert.ok(surfaceStyle);
  const forbidden = (): never => {
    throw new Error('feedback-frame-rediscovery');
  };
  try {
    Object.defineProperty(surface, 'style', {
      configurable: true,
      get: forbidden,
    });
    window.getComputedStyle = forbidden;
    c.feedback.follow(
      { x: 40, y: 50 },
      () => true,
      (callback) => callback(),
    );
    c.f.frame(16);
    assert.equal(style.transform, 'translate(36px, 48px)');
    assert.equal(style.top, '0px');
    assert.equal(c.f.frames.size, 0);
  } finally {
    Object.defineProperty(surface, 'style', surfaceStyle);
    window.getComputedStyle = computed;
    c.feedback.close();
    c.f.destroy();
  }
});

test('failure after currentness reentry preserves its raw cause without retiring the newer hold', () => {
  const c = feedbackFixture();
  try {
    c.begin();
    let newer: ReturnType<typeof c.begin> | undefined;
    c.feedback.follow(
      { x: 40, y: 50 },
      () => {
        newer = c.begin();
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
        throw undefined;
      },
      (callback) => callback(),
    );
    const causes: unknown[] = [];
    try {
      c.f.frame(16);
    } catch (cause) {
      causes.push(cause);
    }
    assert.equal(causes.length, 1);
    assert.equal(causes[0], undefined);
    assert.ok(newer);
    assert.equal(newer.surface.removed, false);
    assert.equal(newer.surface.style.transform, 'translate(10px, 10px)');
    assert.equal(c.f.frames.size, 0);
  } finally {
    c.feedback.close();
    c.f.destroy();
  }
});
