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
/** Characterize viewport measurement and owned, coalesced geometry notifications. */

import { jest } from '@jest/globals';
import { CanvasSurfaceGeometryObserver } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfaceGeometryObserver.js';
import { BrowserCanvasWorkspaceMotion } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasWorkspaceMotion.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';
import { installCanvasWorkspaceAnimation } from '../support/CanvasWorkspaceMotionFixture.js';

function surface(): HTMLDivElement {
  const element = document.createElement('div');
  element.setAttribute('data-sc-canvas-geometry', '');
  return element;
}
function bounds(left: number, top: number, width: number, height: number): DOMRect {
  return {
    x: left,
    y: top,
    left,
    top,
    right: left + width,
    bottom: top + height,
    width,
    height,
    toJSON: () => ({}),
  } as DOMRect;
}

test('reads immutable client geometry, display density, and actual focus', () => {
  const element = surface();
  element.tabIndex = 0;
  document.body.append(element);
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(20, 30, 400, 200));
  jest.replaceProperty(window, 'devicePixelRatio', 1.5);
  const observer = new CanvasSurfaceGeometryObserver(element, new CanvasFrameFixture(), () => {});
  try {
    element.focus();
    const geometry = observer.read();
    expect(geometry).toEqual({
      coordinateSpace: 'client',
      units: 'css-pixels',
      origin: { x: 20, y: 30 },
      viewport: { width: 400, height: 200 },
      clip: { x: 20, y: 30, width: 400, height: 200 },
      physicalPixelsPerCssPixel: 1.5,
      visible: true,
      focused: true,
    });
    expect(Object.isFrozen(geometry)).toBe(true);
    expect(Object.isFrozen(geometry.clip)).toBe(true);
  } finally {
    observer.dispose();
    element.remove();
    jest.restoreAllMocks();
  }
});

test('coalesces geometry notifications and retires pending callbacks', () => {
  const element = surface();
  document.body.append(element);
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(0, 0, 200, 100));
  const frames = new CanvasFrameFixture();
  const changed = jest.fn();
  const observer = new CanvasSurfaceGeometryObserver(element, frames, changed);
  try {
    window.dispatchEvent(new Event('resize'));
    window.dispatchEvent(new Event('resize'));
    element.dispatchEvent(new Event('focusin', { bubbles: true }));
    expect(frames.pending).toBe(1);
    frames.advance(16);
    expect(changed).toHaveBeenCalledTimes(1);
    window.dispatchEvent(new Event('resize'));
    expect(frames.pending).toBe(1);
    observer.dispose();
    expect(frames.pending).toBe(0);
    window.dispatchEvent(new Event('resize'));
    frames.advance(16);
    expect(changed).toHaveBeenCalledTimes(1);
  } finally {
    observer.dispose();
    element.remove();
    jest.restoreAllMocks();
  }
});

test('tracks a running ancestor animation until it settles, then stops scheduling frames', () => {
  const parent = document.createElement('div');
  const element = surface();
  parent.append(element);
  document.body.append(parent);
  let left = 10;
  let running = true;
  jest.spyOn(element, 'getBoundingClientRect').mockImplementation(() => bounds(left, 20, 200, 100));
  Object.defineProperty(parent, 'getAnimations', {
    configurable: true,
    value: () => (running ? [{ playState: 'running', pending: false }] : []),
  });
  const frames = new CanvasFrameFixture();
  const changed = jest.fn<(geometry: ReturnType<CanvasSurfaceGeometryObserver['read']>) => void>();
  const observer = new CanvasSurfaceGeometryObserver(element, frames, changed);
  try {
    expect(frames.pending).toBe(1);
    frames.advance(16);
    expect(changed.mock.calls.at(-1)?.[0].origin.x).toBe(10);
    expect(frames.pending).toBe(1);
    left = 35;
    frames.advance(16);
    expect(changed.mock.calls.at(-1)?.[0].origin.x).toBe(35);
    running = false;
    frames.advance(16);
    expect(frames.pending).toBe(0);
    frames.advance(16);
    expect(changed).toHaveBeenCalledTimes(3);
  } finally {
    observer.dispose();
    parent.remove();
    jest.restoreAllMocks();
  }
});

test('starts and retires ancestor transition tracking without observing descendant animations', () => {
  const parent = document.createElement('div');
  const element = surface();
  const descendant = document.createElement('div');
  parent.append(element);
  element.append(descendant);
  document.body.append(parent);
  let running = false;
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(10, 20, 200, 100));
  Object.defineProperty(parent, 'getAnimations', {
    configurable: true,
    value: () => (running ? [{ playState: 'running', pending: false }] : []),
  });
  const frames = new CanvasFrameFixture();
  const changed = jest.fn();
  const observer = new CanvasSurfaceGeometryObserver(element, frames, changed);
  try {
    descendant.dispatchEvent(new Event('animationstart', { bubbles: true }));
    expect(frames.pending).toBe(0);
    running = true;
    parent.dispatchEvent(new Event('transitionrun', { bubbles: true }));
    expect(frames.pending).toBe(1);
    frames.advance(16);
    expect(frames.pending).toBe(1);
    running = false;
    parent.dispatchEvent(new Event('transitionend', { bubbles: true }));
    frames.advance(16);
    expect(frames.pending).toBe(0);
    running = true;
    parent.dispatchEvent(new Event('animationstart', { bubbles: true }));
    observer.dispose();
    expect(frames.pending).toBe(0);
    parent.dispatchEvent(new Event('transitionrun', { bubbles: true }));
    expect(frames.pending).toBe(0);
  } finally {
    observer.dispose();
    parent.remove();
    jest.restoreAllMocks();
  }
});

test("tracks the workspace owner's programmatic exit slide before its visibility settles", async () => {
  const pane = document.createElement('aside');
  const element = surface();
  pane.append(element);
  document.body.append(pane);
  const effect = installCanvasWorkspaceAnimation(pane);
  const motion = new BrowserCanvasWorkspaceMotion(pane, () => {});
  motion.restore(true);
  let left = 10;
  jest.spyOn(element, 'getBoundingClientRect').mockImplementation(() => bounds(left, 20, 200, 100));
  Object.defineProperty(pane, 'getAnimations', {
    configurable: true,
    value: () => (pane.dataset.canvasMotion ? [{ playState: 'running', pending: false }] : []),
  });
  const frames = new CanvasFrameFixture();
  const changed = jest.fn<(geometry: ReturnType<CanvasSurfaceGeometryObserver['read']>) => void>();
  const observer = new CanvasSurfaceGeometryObserver(element, frames, changed);
  try {
    const exiting = motion.setVisible(false);
    await Promise.resolve();
    expect(frames.pending).toBe(1);
    frames.advance(16);
    left = 70;
    frames.advance(16);
    expect(changed.mock.calls.at(-1)?.[0].origin.x).toBe(70);
    effect.effects[0]!.finish();
    await exiting;
    await Promise.resolve();
    frames.advance(16);
    expect(changed.mock.calls.at(-1)?.[0].visible).toBe(false);
    expect(frames.pending).toBe(0);
  } finally {
    observer.dispose();
    motion.dispose();
    pane.remove();
    jest.restoreAllMocks();
  }
});
test('retires motion listeners for old ancestors after the surface is moved', async () => {
  const first = document.createElement('div');
  const second = document.createElement('div');
  const element = surface();
  first.append(element);
  document.body.append(first, second);
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(10, 20, 200, 100));
  const frames = new CanvasFrameFixture();
  const observer = new CanvasSurfaceGeometryObserver(element, frames, () => {});
  try {
    second.append(element);
    await Promise.resolve();
    frames.advance(16);
    expect(frames.pending).toBe(0);
    first.dispatchEvent(new Event('transitionrun'));
    expect(frames.pending).toBe(0);
    second.dispatchEvent(new Event('transitionrun'));
    expect(frames.pending).toBe(1);
    frames.advance(16);
    expect(frames.pending).toBe(0);
  } finally {
    observer.dispose();
    first.remove();
    second.remove();
    jest.restoreAllMocks();
  }
});

test('clips the native allocation to the inner area of scrolling ancestors', () => {
  const parent = document.createElement('div');
  const element = surface();
  parent.style.overflow = 'hidden';
  parent.append(element);
  document.body.append(parent);
  jest.spyOn(parent, 'getBoundingClientRect').mockReturnValue(bounds(30, 40, 130, 90));
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(10, 20, 200, 150));
  Object.defineProperties(parent, {
    clientLeft: { configurable: true, value: 5 },
    clientTop: { configurable: true, value: 5 },
    clientWidth: { configurable: true, value: 120 },
    clientHeight: { configurable: true, value: 80 },
    offsetWidth: { configurable: true, value: 130 },
    offsetHeight: { configurable: true, value: 90 },
  });
  const observer = new CanvasSurfaceGeometryObserver(element, new CanvasFrameFixture(), () => {});
  try {
    expect(observer.read().clip).toEqual({ x: 35, y: 45, width: 120, height: 80 });
  } finally {
    observer.dispose();
    parent.remove();
    jest.restoreAllMocks();
  }
});

test('updates clipped geometry when an ancestor scrolls and cancels notifications on disposal', () => {
  const parent = document.createElement('div');
  const element = surface();
  parent.append(element);
  document.body.append(parent);
  let left = 40;
  jest.spyOn(element, 'getBoundingClientRect').mockImplementation(() => bounds(left, 20, 200, 100));
  const frames = new CanvasFrameFixture();
  const changed = jest.fn<(geometry: ReturnType<CanvasSurfaceGeometryObserver['read']>) => void>();
  const observer = new CanvasSurfaceGeometryObserver(element, frames, changed);
  try {
    left = 10;
    parent.dispatchEvent(new Event('scroll'));
    expect(frames.pending).toBe(1);
    frames.advance(16);
    expect(changed).toHaveBeenCalledTimes(1);
    expect(changed.mock.calls[0]?.[0].origin.x).toBe(10);
    observer.dispose();
    parent.dispatchEvent(new Event('scroll'));
    expect(frames.pending).toBe(0);
  } finally {
    observer.dispose();
    parent.remove();
    jest.restoreAllMocks();
  }
});

test('hides a staged surface until its ancestor becomes visible', async () => {
  const parent = document.createElement('div');
  const element = surface();
  parent.style.visibility = 'hidden';
  parent.append(element);
  document.body.append(parent);
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(10, 20, 200, 100));
  const frames = new CanvasFrameFixture();
  const changed = jest.fn<(geometry: ReturnType<CanvasSurfaceGeometryObserver['read']>) => void>();
  const observer = new CanvasSurfaceGeometryObserver(element, frames, changed);
  try {
    expect(observer.read().visible).toBe(false);
    expect(observer.read().clip.width).toBe(0);
    parent.style.visibility = 'visible';
    await Promise.resolve();
    expect(frames.pending).toBe(1);
    frames.advance(16);
    expect(changed.mock.calls[0]?.[0].visible).toBe(true);
  } finally {
    observer.dispose();
    parent.remove();
    jest.restoreAllMocks();
  }
});

test('does not expose native pixels through a non-rectangular clip', () => {
  const parent = document.createElement('div');
  const element = surface();
  parent.style.overflow = 'hidden';
  parent.style.borderRadius = '12px';
  parent.append(element);
  document.body.append(parent);
  jest.spyOn(parent, 'getBoundingClientRect').mockReturnValue(bounds(0, 0, 220, 120));
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(10, 10, 200, 100));
  const observer = new CanvasSurfaceGeometryObserver(element, new CanvasFrameFixture(), () => {});
  try {
    const geometry = observer.read();
    expect(geometry.visible).toBe(true);
    expect(geometry.clip.width).toBe(0);
    expect(geometry.clip.height).toBe(0);
  } finally {
    observer.dispose();
    parent.remove();
    jest.restoreAllMocks();
  }
});
test('intersects the browser viewport without changing the canvas allocation', () => {
  const element = surface();
  document.body.append(element);
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(-50, 740, 200, 100));
  const observer = new CanvasSurfaceGeometryObserver(element, new CanvasFrameFixture(), () => {});
  try {
    const geometry = observer.read();
    expect(geometry.origin).toEqual({ x: -50, y: 740 });
    expect(geometry.viewport).toEqual({ width: 200, height: 100 });
    expect(geometry.clip).toEqual({ x: 0, y: 740, width: 150, height: 28 });
  } finally {
    observer.dispose();
    element.remove();
    jest.restoreAllMocks();
  }
});

test('rejects an unrepresentable transform without hiding the browser surface', () => {
  const parent = document.createElement('div');
  const element = surface();
  parent.style.transform = 'rotate(15deg)';
  parent.append(element);
  document.body.append(parent);
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(10, 20, 200, 100));
  const observer = new CanvasSurfaceGeometryObserver(element, new CanvasFrameFixture(), () => {});
  try {
    const geometry = observer.read();
    expect(geometry.visible).toBe(true);
    expect(geometry.clip).toEqual({ x: 10, y: 20, width: 0, height: 0 });
  } finally {
    observer.dispose();
    parent.remove();
    jest.restoreAllMocks();
  }
});

test('uses a hidden safe geometry when the browser reports an invalid density', () => {
  const element = surface();
  document.body.append(element);
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(10, 20, 200, 100));
  jest.replaceProperty(window, 'devicePixelRatio', 0);
  const observer = new CanvasSurfaceGeometryObserver(element, new CanvasFrameFixture(), () => {});
  try {
    expect(observer.read()).toMatchObject({
      origin: { x: 0, y: 0 },
      viewport: { width: 0, height: 0 },
      clip: { x: 0, y: 0, width: 0, height: 0 },
      physicalPixelsPerCssPixel: 1,
      visible: false,
    });
  } finally {
    observer.dispose();
    element.remove();
    jest.restoreAllMocks();
  }
});

test('reobserves a moved surface and its new clipping ancestors', async () => {
  const first = document.createElement('div');
  const second = document.createElement('div');
  const element = surface();
  first.append(element);
  document.body.append(first, second);
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(10, 20, 200, 100));
  const frames = new CanvasFrameFixture();
  const changed = jest.fn<(geometry: ReturnType<CanvasSurfaceGeometryObserver['read']>) => void>();
  const observer = new CanvasSurfaceGeometryObserver(element, frames, changed);
  try {
    second.append(element);
    await Promise.resolve();
    expect(frames.pending).toBe(1);
    frames.advance(16);
    expect(changed.mock.calls.at(-1)?.[0].visible).toBe(true);
    second.style.visibility = 'hidden';
    await Promise.resolve();
    expect(frames.pending).toBe(1);
    frames.advance(16);
    expect(changed.mock.calls.at(-1)?.[0].visible).toBe(false);
  } finally {
    observer.dispose();
    first.remove();
    second.remove();
    jest.restoreAllMocks();
  }
});
test('tracks a surface moved between shadow roots and observes the new host', async () => {
  const firstHost = document.createElement('div');
  const secondHost = document.createElement('div');
  const first = firstHost.attachShadow({ mode: 'open' });
  const second = secondHost.attachShadow({ mode: 'open' });
  const element = surface();
  first.append(element);
  document.body.append(firstHost, secondHost);
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(10, 20, 200, 100));
  const frames = new CanvasFrameFixture();
  const changed = jest.fn<(geometry: ReturnType<CanvasSurfaceGeometryObserver['read']>) => void>();
  const observer = new CanvasSurfaceGeometryObserver(element, frames, changed);
  try {
    second.append(element);
    await Promise.resolve();
    expect(frames.pending).toBe(1);
    frames.advance(16);
    secondHost.style.visibility = 'hidden';
    await Promise.resolve();
    expect(frames.pending).toBe(1);
    frames.advance(16);
    expect(changed.mock.calls.at(-1)?.[0].visible).toBe(false);
  } finally {
    observer.dispose();
    firstHost.remove();
    secondHost.remove();
    jest.restoreAllMocks();
  }
});

test('tracks the clipping ancestry when a surface moves between shadow slots', async () => {
  const host = document.createElement('div');
  const root = host.attachShadow({ mode: 'open' });
  const first = document.createElement('div');
  const second = document.createElement('div');
  const firstSlot = document.createElement('slot');
  const secondSlot = document.createElement('slot');
  firstSlot.name = 'first';
  secondSlot.name = 'second';
  first.style.overflow = 'hidden';
  second.style.overflow = 'hidden';
  first.append(firstSlot);
  second.append(secondSlot);
  root.append(first, second);
  const element = surface();
  element.slot = 'first';
  host.append(element);
  document.body.append(host);
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(0, 0, 200, 100));
  jest.spyOn(first, 'getBoundingClientRect').mockReturnValue(bounds(10, 10, 120, 70));
  jest.spyOn(second, 'getBoundingClientRect').mockReturnValue(bounds(40, 20, 100, 60));
  for (const [node, width, height] of [
    [first, 120, 70],
    [second, 100, 60],
  ] as const) {
    Object.defineProperties(node, {
      clientWidth: { configurable: true, value: width },
      clientHeight: { configurable: true, value: height },
      offsetWidth: { configurable: true, value: width },
      offsetHeight: { configurable: true, value: height },
    });
  }
  const frames = new CanvasFrameFixture();
  const changed = jest.fn<(geometry: ReturnType<CanvasSurfaceGeometryObserver['read']>) => void>();
  const observer = new CanvasSurfaceGeometryObserver(element, frames, changed);
  try {
    expect(observer.read().clip).toEqual({ x: 10, y: 10, width: 120, height: 70 });
    element.slot = 'second';
    await Promise.resolve();
    expect(frames.pending).toBe(1);
    frames.advance(16);
    expect(changed.mock.calls.at(-1)?.[0].clip).toEqual({ x: 40, y: 20, width: 100, height: 60 });
    second.style.visibility = 'hidden';
    await Promise.resolve();
    expect(frames.pending).toBe(1);
    frames.advance(16);
    expect(changed.mock.calls.at(-1)?.[0].visible).toBe(false);
    secondSlot.dispatchEvent(new Event('slotchange'));
    expect(frames.pending).toBe(1);
    observer.dispose();
    expect(frames.pending).toBe(0);
    secondSlot.dispatchEvent(new Event('slotchange'));
    expect(frames.pending).toBe(0);
  } finally {
    observer.dispose();
    host.remove();
    jest.restoreAllMocks();
  }
});

test('accepts the rectangular inset clip used by the Comfy graph container', () => {
  const parent = document.createElement('div');
  const element = surface();
  parent.style.clipPath = 'inset(-78px -512px -4px 0px)';
  parent.append(element);
  document.body.append(parent);
  jest.spyOn(parent, 'getBoundingClientRect').mockReturnValue(bounds(0, 78, 768, 718));
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(770, 78, 510, 718));
  Object.defineProperties(parent, {
    offsetWidth: { configurable: true, value: 768 },
    offsetHeight: { configurable: true, value: 718 },
  });
  jest.replaceProperty(window, 'innerWidth', 1280);
  jest.replaceProperty(window, 'innerHeight', 800);
  const observer = new CanvasSurfaceGeometryObserver(element, new CanvasFrameFixture(), () => {});
  try {
    expect(observer.read().clip).toEqual({ x: 770, y: 78, width: 509, height: 718 });
  } finally {
    observer.dispose();
    parent.remove();
    jest.restoreAllMocks();
  }
});

test('rejects inset clips with rounded or unsupported shapes', () => {
  const parent = document.createElement('div');
  const element = surface();
  parent.style.clipPath = 'inset(0px round 12px)';
  parent.append(element);
  document.body.append(parent);
  jest.spyOn(parent, 'getBoundingClientRect').mockReturnValue(bounds(0, 0, 200, 100));
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(0, 0, 200, 100));
  Object.defineProperties(parent, {
    offsetWidth: { configurable: true, value: 200 },
    offsetHeight: { configurable: true, value: 100 },
  });
  const observer = new CanvasSurfaceGeometryObserver(element, new CanvasFrameFixture(), () => {});
  try {
    expect(observer.read()).toMatchObject({ visible: true, clip: { width: 0, height: 0 } });
  } finally {
    observer.dispose();
    parent.remove();
    jest.restoreAllMocks();
  }
});

test('finds a surface reattached after an asynchronous detach', async () => {
  const first = document.createElement('div');
  const second = document.createElement('div');
  const element = surface();
  first.append(element);
  document.body.append(first, second);
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(10, 20, 200, 100));
  const frames = new CanvasFrameFixture();
  const changed = jest.fn<(geometry: ReturnType<CanvasSurfaceGeometryObserver['read']>) => void>();
  const observer = new CanvasSurfaceGeometryObserver(element, frames, changed);
  try {
    element.remove();
    await Promise.resolve();
    frames.advance(16);
    expect(changed.mock.calls.at(-1)?.[0].visible).toBe(false);
    second.append(element);
    await Promise.resolve();
    expect(frames.pending).toBe(1);
    frames.advance(16);
    expect(changed.mock.calls.at(-1)?.[0].visible).toBe(true);
  } finally {
    observer.dispose();
    first.remove();
    second.remove();
    jest.restoreAllMocks();
  }
});

test('resolves percentage inset edges against the clipping box', () => {
  const parent = document.createElement('div');
  const element = surface();
  parent.style.clipPath = 'inset(10% 20% 15% 5%)';
  parent.append(element);
  document.body.append(parent);
  jest.spyOn(parent, 'getBoundingClientRect').mockReturnValue(bounds(100, 50, 400, 200));
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(100, 50, 400, 200));
  Object.defineProperties(parent, {
    offsetWidth: { configurable: true, value: 400 },
    offsetHeight: { configurable: true, value: 200 },
  });
  const observer = new CanvasSurfaceGeometryObserver(element, new CanvasFrameFixture(), () => {});
  try {
    expect(observer.read().clip).toEqual({ x: 121, y: 71, width: 298, height: 148 });
  } finally {
    observer.dispose();
    parent.remove();
    jest.restoreAllMocks();
  }
});

test('keeps an empty intersection within the bounded viewport', () => {
  const parent = document.createElement('div');
  const element = surface();
  parent.style.clipPath = 'inset(0px 0px 0px 2000000px)';
  parent.append(element);
  document.body.append(parent);
  jest.spyOn(parent, 'getBoundingClientRect').mockReturnValue(bounds(10, 20, 200, 100));
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(10, 20, 200, 100));
  Object.defineProperties(parent, {
    offsetWidth: { configurable: true, value: 200 },
    offsetHeight: { configurable: true, value: 100 },
  });
  const observer = new CanvasSurfaceGeometryObserver(element, new CanvasFrameFixture(), () => {});
  try {
    expect(observer.read().clip).toEqual({ x: 10, y: 20, width: 0, height: 0 });
  } finally {
    observer.dispose();
    parent.remove();
    jest.restoreAllMocks();
  }
});

test('retains surface identity when another geometry observer retires', async () => {
  const first = document.createElement('div');
  const second = document.createElement('div');
  const element = surface();
  first.append(element);
  document.body.append(first, second);
  jest.spyOn(element, 'getBoundingClientRect').mockReturnValue(bounds(10, 20, 200, 100));
  const frames = new CanvasFrameFixture();
  const changed = jest.fn<(geometry: ReturnType<CanvasSurfaceGeometryObserver['read']>) => void>();
  const retiring = new CanvasSurfaceGeometryObserver(element, new CanvasFrameFixture(), () => {});
  const observer = new CanvasSurfaceGeometryObserver(element, frames, changed);
  try {
    retiring.dispose();
    expect(element.hasAttribute('data-sc-canvas-geometry')).toBe(true);
    element.remove();
    await Promise.resolve();
    frames.advance(16);
    second.append(element);
    await Promise.resolve();
    frames.advance(16);
    expect(changed.mock.calls.at(-1)?.[0].visible).toBe(true);
  } finally {
    retiring.dispose();
    observer.dispose();
    first.remove();
    second.remove();
    jest.restoreAllMocks();
  }
});

test('requires the surface owner to supply geometry identity', () => {
  const element = document.createElement('div');
  expect(
    () => new CanvasSurfaceGeometryObserver(element, new CanvasFrameFixture(), () => {}),
  ).toThrow('Canvas geometry surface is not identified.');
});
