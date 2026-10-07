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
/** Safeguard reversible disclosure and bounded placement without remounting native fields. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { OverrideToolbarDisclosure } from '../../../frontend/comfyui/ui/relations/presentation/OverrideToolbarDisclosure.js';
import { OverrideToolbarDrag } from '../../../frontend/comfyui/ui/relations/presentation/OverrideToolbarDrag.js';
import { OverrideToolbarPlacement } from '../../../frontend/comfyui/ui/relations/presentation/OverrideToolbarPlacement.js';
import { ensureRelationStyles } from '../../../frontend/comfyui/ui/relations/presentation/RelationStyles.js';

const originalMedia = Object.getOwnPropertyDescriptor(window, 'matchMedia');
const originalResize = Object.getOwnPropertyDescriptor(window, 'ResizeObserver');

afterEach(() => {
  jest.restoreAllMocks();
  if (originalMedia) Object.defineProperty(window, 'matchMedia', originalMedia);
  else Reflect.deleteProperty(window, 'matchMedia');
  if (originalResize) Object.defineProperty(window, 'ResizeObserver', originalResize);
  else Reflect.deleteProperty(window, 'ResizeObserver');
  document.body.replaceChildren();
  document.getElementById('sc-relation-styles')?.remove();
});

/** Supply deterministic panel and toolbar bounds without browser layout. */
function bounds(x: number, y: number, width: number, height: number): DOMRect {
  return {
    x,
    y,
    left: x,
    top: y,
    right: x + width,
    bottom: y + height,
    width,
    height,
    toJSON: () => ({}),
  };
}

/** Keep a mounted native input inside the strip throughout disclosure. */
function surface() {
  const panel = document.createElement('div');
  const root = document.createElement('div');
  root.className = 'sc-overrides-toolbar';
  const fields = document.createElement('div');
  const input = document.createElement('input');
  const button = document.createElement('button');
  const grip = document.createElement('button');
  grip.className = 'sc-overrides-grip';
  fields.append(input);
  root.append(grip, fields, button);
  panel.append(root);
  document.body.append(panel);
  panel.getBoundingClientRect = () => bounds(0, 0, 800, 600);
  root.getBoundingClientRect = () =>
    bounds(
      Number.parseFloat(root.style.left) || 4,
      Number.parseFloat(root.style.top) || 80,
      fields.hidden ? 100 : 400,
      42,
    );
  grip.getBoundingClientRect = () => {
    const rootBounds = root.getBoundingClientRect();
    return bounds(rootBounds.x, rootBounds.y, 20, 32);
  };
  return { panel, root, fields, input, button, grip };
}

/** Flush completion microtasks without coupling tests to wall-clock animation. */
async function settle(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

test('animates the complete bar, preserves input identity, and excludes compact fields from focus', async () => {
  ensureRelationStyles(document);
  const { root, fields, input, button } = surface();
  const animations: { complete: () => void; cancel: ReturnType<typeof jest.fn> }[] = [];
  const animate = jest.fn(
    (
      _: Keyframe[] | PropertyIndexedKeyframes | null,
      _options?: number | KeyframeAnimationOptions,
    ) => {
      let complete: () => void = () => undefined;
      const finished = new Promise<void>((resolve) => {
        complete = resolve;
      });
      const cancel = jest.fn();
      animations.push({ complete, cancel });
      // The browser adapter only consumes completion and cancellation in these deterministic effects.
      return { finished, cancel } as unknown as Animation;
    },
  );
  Object.defineProperty(root, 'animate', { value: animate });
  Object.defineProperty(fields, 'animate', { value: animate });
  const geometry = jest.fn();
  const error = jest.fn();
  const disclosure = new OverrideToolbarDisclosure(root, fields, button, geometry, error);
  input.focus();
  button.click();
  expect(root.dataset.collapsed).toBe('true');
  expect(fields.inert).toBe(true);
  expect(document.activeElement).toBe(button);
  expect(fields.hidden).toBe(false);
  expect(getComputedStyle(root).overflow).toBe('clip');
  expect(button.getAttribute('aria-label')).toBe('Expand global overrides');
  expect(geometry).toHaveBeenLastCalledWith(100, 42);
  expect(animate.mock.calls[0]?.[0]).toEqual([{ width: '400px' }, { width: '100px' }]);
  expect(animate.mock.calls[0]?.[1]).toMatchObject({ duration: 200, fill: 'both' });
  animations.forEach((effect) => effect.complete());
  await settle();
  expect(fields.hidden).toBe(true);
  expect(root.dataset.scOverrideMoving).toBeUndefined();
  expect(root.hasAttribute('data-sc-geometry-moving')).toBe(false);
  button.click();
  expect(animate.mock.calls[3]?.[0]).toEqual([
    expect.objectContaining({ opacity: '0' }),
    expect.objectContaining({ opacity: 1 }),
  ]);
  expect(fields.inert).toBe(false);
  expect(fields.firstElementChild).toBe(input);
  animations.forEach((effect) => effect.complete());
  await settle();
  expect(fields.hidden).toBe(false);
  expect(error).not.toHaveBeenCalled();
  disclosure.dispose();
});

test('reversals cancel stale completion and teardown releases every animation', async () => {
  const { root, fields, button } = surface();
  const effects: { complete: () => void; cancel: ReturnType<typeof jest.fn> }[] = [];
  const animate = () => {
    let complete: () => void = () => undefined;
    const finished = new Promise<void>((resolve) => {
      complete = resolve;
    });
    const cancel = jest.fn();
    effects.push({ complete, cancel });
    // The browser adapter only consumes completion and cancellation in these deterministic effects.
    return { finished, cancel } as unknown as Animation;
  };
  Object.defineProperty(root, 'animate', { value: animate });
  Object.defineProperty(fields, 'animate', { value: animate });
  const error = jest.fn();
  const disclosure = new OverrideToolbarDisclosure(root, fields, button, jest.fn(), error);
  button.click();
  button.click();
  effects[0]?.complete();
  effects[1]?.complete();
  await settle();
  expect(fields.hidden).toBe(false);
  expect(root.dataset.scOverrideMoving).toBe('');
  expect(root.hasAttribute('data-sc-geometry-moving')).toBe(true);
  expect(effects[0]?.cancel).toHaveBeenCalledTimes(1);
  disclosure.dispose();
  expect(effects[2]?.cancel).toHaveBeenCalledTimes(1);
  expect(root.dataset.scOverrideMoving).toBeUndefined();
  expect(root.hasAttribute('data-sc-geometry-moving')).toBe(false);
  expect(error).not.toHaveBeenCalled();
});

test('reduced motion settles immediately and failed effects restore usable controls', () => {
  const { root, fields, button } = surface();
  const animate = jest.fn(() => {
    throw new Error('animation failed');
  });
  Object.defineProperty(root, 'animate', { value: animate });
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    value: () => ({ matches: true }),
  });
  const error = jest.fn();
  const disclosure = new OverrideToolbarDisclosure(root, fields, button, jest.fn(), error);
  button.click();
  expect(fields.hidden).toBe(true);
  expect(animate).not.toHaveBeenCalled();
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    value: () => ({ matches: false }),
  });
  button.click();
  expect(fields.hidden).toBe(false);
  expect(root.dataset.scOverrideMoving).toBeUndefined();
  expect(root.hasAttribute('data-sc-geometry-moving')).toBe(false);
  expect(error).toHaveBeenCalledWith(expect.objectContaining({ message: 'animation failed' }));
  disclosure.dispose();
});

test('collapsing transfers keyboard focus from the disappearing grip to the expansion button', () => {
  const { root, fields, grip, button } = surface();
  const disclosure = new OverrideToolbarDisclosure(root, fields, button, jest.fn(), jest.fn());
  grip.focus();
  button.click();
  expect(document.activeElement).toBe(button);
  expect(root.dataset.collapsed).toBe('true');
  disclosure.dispose();
});

/** Deliver pointer coordinates independently of jsdom's missing pointer capture implementation. */
function pointer(target: HTMLElement, type: string, x: number, y: number, button = 0): void {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.defineProperties(event, {
    pointerId: { value: 1 },
    clientX: { value: x },
    clientY: { value: y },
    button: { value: button },
  });
  target.dispatchEvent(event);
}

/** Keep the established floating geometry proofs independent of native docking discovery. */
function floatingPlacement(root: HTMLElement, grip: HTMLElement): OverrideToolbarPlacement {
  return new OverrideToolbarPlacement(
    root,
    grip,
    {
      attach: () => true,
      canDock: () => false,
      dockBounds: () => null,
      floatingBounds: () => ({ bounds: bounds(0, 40, 800, 560), origin: { x: 4, y: 80 } }),
      dragging: jest.fn(),
      preview: jest.fn(),
      dispose: jest.fn(),
    },
    jest.fn(),
    jest.fn(),
  );
}

test('grip dragging clamps below tabs, leaves widgets alone, and retains placement after reattachment', () => {
  const { panel, root, fields, grip } = surface();
  const begin = jest.fn();
  const capture = jest.fn();
  const release = jest.fn();
  Object.defineProperties(grip, {
    setPointerCapture: { value: capture },
    hasPointerCapture: { value: () => true },
    releasePointerCapture: { value: release },
  });
  const placement = floatingPlacement(root, grip);
  const drag = new OverrideToolbarDrag(root, grip, placement, begin);
  placement.attached();
  placement.resize(400, 42);
  pointer(fields, 'pointerdown', 20, 90);
  expect(begin).not.toHaveBeenCalled();
  pointer(grip, 'pointerdown', 20, 90, 2);
  expect(begin).not.toHaveBeenCalled();
  pointer(grip, 'pointerdown', 20, 90);
  pointer(grip, 'pointermove', 900, 900);
  expect(root.style.left).toBe('708px');
  expect(root.style.top).toBe('554px');
  expect(root.style.transform).toBe('');
  expect(capture).toHaveBeenCalledWith(1);
  pointer(grip, 'pointercancel', 900, 900);
  expect(release).toHaveBeenCalledWith(1);
  expect(root.dataset.scOverrideDragging).toBeUndefined();
  const replacement = document.createElement('div');
  replacement.getBoundingClientRect = panel.getBoundingClientRect;
  document.body.append(replacement);
  replacement.append(root);
  placement.attached();
  placement.resize(400, 42);
  expect(root.style.left).toBe('708px');
  expect(root.style.top).toBe('554px');
  pointer(grip, 'pointerdown', 20, 90);
  pointer(grip, 'pointermove', -900, -900);
  expect(root.style.left).toBe('4px');
  expect(root.style.top).toBe('40px');
  drag.dispose();
  placement.dispose();
});

test('keyboard movement and settled disclosure widths share one bounded position owner', () => {
  const { root, grip } = surface();
  const begin = jest.fn();
  const placement = floatingPlacement(root, grip);
  const drag = new OverrideToolbarDrag(root, grip, placement, begin);
  placement.attached();
  placement.resize(100, 42);
  for (let index = 0; index < 20; index++)
    grip.dispatchEvent(
      new KeyboardEvent('keydown', {
        key: 'ArrowRight',
        shiftKey: true,
        bubbles: true,
        cancelable: true,
      }),
    );
  expect(root.style.left).toBe('696px');
  placement.resize(400, 42);
  expect(root.style.left).toBe('396px');
  placement.resize(100, 42);
  expect(root.style.left).toBe('696px');
  grip.dispatchEvent(new KeyboardEvent('keydown', { key: 'Home' }));
  expect(root.style.left).toBe('4px');
  expect(root.style.top).toBe('80px');
  drag.dispose();
  placement.dispose();
  grip.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
  expect(root.style.left).toBe('4px');
});

test('host resize coalesces placement and observes only the panel, while disposal cancels queued work', () => {
  const { panel, root, grip } = surface();
  let resized: ResizeObserverCallback | undefined;
  let browserObserver: ResizeObserver | undefined;
  const observe = jest.fn();
  const disconnect = jest.fn();
  /** Expose the host geometry callback without observing toolbar animation. */
  class PanelObserver implements ResizeObserver {
    /** Capture the browser callback at the external geometry boundary. */
    constructor(callback: ResizeObserverCallback) {
      resized = callback;
      browserObserver = this;
    }
    observe = observe;
    disconnect = disconnect;
    unobserve = jest.fn();
  }
  Object.defineProperty(window, 'ResizeObserver', { configurable: true, value: PanelObserver });
  const frames: FrameRequestCallback[] = [];
  const schedule = jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  const cancel = jest.spyOn(window, 'cancelAnimationFrame').mockImplementation(() => undefined);
  const placement = floatingPlacement(root, grip);
  const drag = new OverrideToolbarDrag(root, grip, placement, jest.fn());
  placement.attached();
  placement.resize(400, 42);
  expect(observe).toHaveBeenCalledTimes(1);
  expect(observe).toHaveBeenCalledWith(panel);
  if (!browserObserver) throw new Error('Panel observer was not attached');
  resized?.([], browserObserver);
  resized?.([], browserObserver);
  expect(schedule).toHaveBeenCalledTimes(1);
  placement.resize(100, 42);
  for (let index = 0; index < 20; index++)
    grip.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', shiftKey: true }));
  expect(root.style.left).toBe('696px');
  frames[0]?.(0);
  expect(root.style.left).toBe('396px');
  placement.attached();
  expect(observe).toHaveBeenCalledTimes(1);
  resized?.([], browserObserver);
  drag.dispose();
  placement.dispose();
  expect(cancel).toHaveBeenCalledWith(2);
  expect(disconnect).toHaveBeenCalledTimes(1);
});
