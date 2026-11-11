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
/** Native scroller clipping is a fresh READ admission separate from unchanged mount geometry and cached plans. */
import assert from 'node:assert/strict';
import test from 'node:test';
import {
  attachReorderMotionStage,
  type ReorderMotionLayerLease,
} from '../../../../../../frontend/sugarprompt/editor/src/reorder-motion-layer.ts';
import { motionPaintFixture } from './reorder-motion-paint-fixture.ts';

/** A genuine attachment lease starts before or after its calibration READ; partial acquisition retains every cleanup owner. */
function fixture(ready = true) {
  const f = motionPaintFixture();
  let node: ReturnType<typeof f.document.createElement> | null = null,
    lease: ReorderMotionLayerLease | null = null;
  try {
    node = f.document.createElement('span');
    node.className = 'sp-reorder-motion-stage';
    node.style.width = '200px';
    node.style.height = '256px';
    lease = attachReorderMotionStage(f.view, node as unknown as HTMLElement, 200, () => {});
    assert.ok(lease);
    if (ready) f.drawLayer();
    return { f, node, lease };
  } catch (primary) {
    retire([() => lease?.release(), () => node?.remove(), () => f.destroyLayer()], [primary]);
    throw primary;
  }
}

/** Every owner retires independently after the original body cause, including raw null/undefined. */
function retire(actions: readonly (() => void)[], causes: unknown[]): void {
  for (const action of actions)
    try {
      action();
    } catch (cause) {
      causes.push(cause);
    }
  if (causes.length === 1) throw causes[0];
  if (causes.length) throw new AggregateError(causes, 'native-clip-admission-proof-failed');
}

for (const axis of ['overflow-x', 'overflow-y'] as const) {
  for (const value of ['visible', 'hidden', 'clip', 'scroll'] as const) {
    test(`native ${axis}=${value} refuses an unmeasured borrowed viewport`, () => {
      const { f, node, lease } = fixture(false),
        native = f.document.defaultView.getComputedStyle.bind(f.document.defaultView),
        causes: unknown[] = [];
      try {
        f.document.defaultView.getComputedStyle = (element, pseudo) => {
          const style = native(element, pseudo);
          return Object.is(element, f.scroller)
            ? {
                ...style,
                getPropertyValue: (name: string) =>
                  name === axis ? value : style.getPropertyValue(name),
              }
            : style;
        };
        assert.equal(lease.read(), 'unavailable');
        f.drawLayer();
        assert.equal(lease.read(), 'unavailable');
        assert.equal(f.layerMount.style.width, '0px');
        assert.equal(f.layerMount.style.height, '0px');
        assert.equal(f.layerMount.style.overflow, 'clip');
      } catch (cause) {
        causes.push(cause);
      }
      retire(
        [
          () => {
            f.document.defaultView.getComputedStyle = native;
          },
          () => lease.release(),
          () => node.remove(),
          () => f.destroyLayer(),
        ],
        causes,
      );
    });
  }

  test(`cached calibration rechecks actual native ${axis} before reuse and ready publication`, () => {
    const { f, node, lease } = fixture(),
      native = f.document.defaultView.getComputedStyle.bind(f.document.defaultView),
      geometry = node.getBoundingClientRect.bind(node),
      causes: unknown[] = [];
    let changed = false,
      reads = 0,
      staleGeometry = 0;
    try {
      assert.equal(lease.read(), 'ready');
      assert.equal(f.layerMount.style.width, '240px');
      assert.equal(f.layerMount.style.height, '100px');
      assert.equal(f.layerMount.style.overflow, 'visible');
      f.document.defaultView.getComputedStyle = (element, pseudo) => {
        const style = native(element, pseudo);
        return Object.is(element, f.scroller)
          ? {
              ...style,
              getPropertyValue: (name: string) => {
                if (name === axis) {
                  reads++;
                  return changed ? 'visible' : 'auto';
                }
                return style.getPropertyValue(name);
              },
            }
          : style;
      };
      node.getBoundingClientRect = () => {
        staleGeometry++;
        return geometry();
      };
      f.drawLayer();
      assert.equal(lease.read(), 'ready');
      assert.ok(reads > 0);
      assert.equal(staleGeometry, 0);
      changed = true;
      assert.equal(lease.read(), 'unavailable');
      f.drawLayer();
      assert.equal(staleGeometry, 0);
      assert.equal(f.layerMount.style.width, '0px');
      assert.equal(f.layerMount.style.overflow, 'clip');
    } catch (cause) {
      causes.push(cause);
    }
    retire(
      [
        () => {
          f.document.defaultView.getComputedStyle = native;
        },
        () => {
          node.getBoundingClientRect = geometry;
        },
        () => lease.release(),
        () => node.remove(),
        () => f.destroyLayer(),
      ],
      causes,
    );
  });
}

for (const action of ['retire', 'detach'] as const) {
  test(`native computed admission cannot certify an attachment after ${action} reentry`, () => {
    const { f, node, lease } = fixture(),
      native = f.document.defaultView.getComputedStyle.bind(f.document.defaultView),
      causes: unknown[] = [];
    let entered = false;
    try {
      f.document.defaultView.getComputedStyle = (element, pseudo) => {
        const style = native(element, pseudo);
        return Object.is(element, f.scroller)
          ? {
              ...style,
              getPropertyValue: (name: string) => {
                const value = style.getPropertyValue(name);
                if (!entered && name === 'overflow-x') {
                  entered = true;
                  if (action === 'retire') f.attachment.destroy();
                  else f.layerRoot.remove();
                }
                return value;
              },
            }
          : style;
      };
      assert.equal(lease.read(), 'unavailable');
      assert.equal(entered, true);
      assert.equal(lease.current(), false);
    } catch (cause) {
      causes.push(cause);
    }
    retire(
      [
        () => {
          f.document.defaultView.getComputedStyle = native;
        },
        () => lease.release(),
        () => node.remove(),
        () => f.destroyLayer(),
      ],
      causes,
    );
  });
}

for (const frontier of ['computed', 'axis'] as const) {
  for (const raw of [null, undefined, Object.freeze({})]) {
    test(`native ${frontier} failure preserves its exact raw cause`, () => {
      const { f, node, lease } = fixture(false),
        native = f.document.defaultView.getComputedStyle.bind(f.document.defaultView),
        causes: unknown[] = [];
      try {
        f.document.defaultView.getComputedStyle = (element, pseudo) => {
          const style = native(element, pseudo);
          if (!Object.is(element, f.scroller)) return style;
          // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
          if (frontier === 'computed') throw raw;
          return {
            ...style,
            getPropertyValue: (name: string) => {
              // eslint-disable-next-line @typescript-eslint/only-throw-error -- Inject raw host failures to prove original cause retention and cleanup.
              if (name === 'overflow-y') throw raw;
              return style.getPropertyValue(name);
            },
          };
        };
        assert.throws(
          () => f.drawLayer(),
          (cause: unknown) => {
            assert.equal(cause, raw);
            return true;
          },
        );
        assert.throws(
          () => lease.read(),
          (cause: unknown) => {
            assert.equal(cause, raw);
            return true;
          },
        );
      } catch (cause) {
        causes.push(cause);
      }
      retire(
        [
          () => {
            f.document.defaultView.getComputedStyle = native;
          },
          () => lease.release(),
          () => node.remove(),
          () => f.destroyLayer(),
        ],
        causes,
      );
    });
  }
}

test('native admission cannot adopt a new borrowed issue then read retired Stage geometry', () => {
  const { f, node, lease } = fixture(false),
    native = f.document.defaultView.getComputedStyle.bind(f.document.defaultView),
    geometry = node.getBoundingClientRect.bind(node),
    next = f.document.createElement('span'),
    causes: unknown[] = [];
  next.className = 'sp-reorder-motion-stage';
  next.style.width = '200px';
  next.style.height = '256px';
  const fresh: { lease: ReorderMotionLayerLease | null } = { lease: null };
  let entered = false,
    oldReads = 0;
  try {
    node.getBoundingClientRect = () => {
      oldReads++;
      throw new Error('RETIRED_STAGE_GEOMETRY_READ');
    };
    f.document.defaultView.getComputedStyle = (element, pseudo) => {
      const style = native(element, pseudo);
      return Object.is(element, f.scroller)
        ? {
            ...style,
            getPropertyValue: (name: string) => {
              const value = style.getPropertyValue(name);
              if (!entered && name === 'overflow-x') {
                entered = true;
                lease.release();
                fresh.lease = attachReorderMotionStage(
                  f.view,
                  next as unknown as HTMLElement,
                  200,
                  () => {},
                );
              }
              return value;
            },
          }
        : style;
    };
    f.drawLayer();
    assert.equal(entered, true);
    assert.equal(oldReads, 0);
    assert.equal(lease.read(), 'unavailable');
    assert.ok(fresh.lease);
    f.document.defaultView.getComputedStyle = native;
    f.drawLayer();
    assert.equal(fresh.lease.read(), 'ready');
    assert.equal(f.layerMount.style.width, '240px');
    assert.equal(f.layerMount.style.height, '100px');
    assert.equal(f.layerMount.style.overflow, 'visible');
  } catch (cause) {
    causes.push(cause);
  }
  retire(
    [
      () => {
        f.document.defaultView.getComputedStyle = native;
      },
      () => {
        node.getBoundingClientRect = geometry;
      },
      () => lease.release(),
      () => fresh.lease?.release(),
      () => node.remove(),
      () => next.remove(),
      () => f.destroyLayer(),
    ],
    causes,
  );
});
