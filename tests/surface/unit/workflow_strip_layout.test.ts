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
/** Prove responsive workflow-strip allocation, stable thresholds and observer retirement. */
import { afterEach, beforeEach, expect, jest, test } from '@jest/globals';
import { WorkflowTabStripLayout } from '../../../frontend/comfyui/ui/surface/frame/WorkflowTabStripLayout.js';

const descriptor = Object.getOwnPropertyDescriptor(window, 'ResizeObserver');
const observers: ResizeHarness[] = [];
const frames = new Map<number, FrameRequestCallback>();

/** Deliver browser resize events without timers or native layout dependencies. */
class ResizeHarness implements ResizeObserver {
  readonly targets = new Set<Element>();
  readonly disconnect = jest.fn(() => this.targets.clear());

  /** Capture the external observer callback for explicit deterministic delivery. */
  constructor(private readonly callback: ResizeObserverCallback) {
    observers.push(this);
  }
  /** Record only the surface whose accepted geometry owns the strip allocation. */
  observe(target: Element): void {
    this.targets.add(target);
  }
  /** Release an individual observed surface. */
  unobserve(target: Element): void {
    this.targets.delete(target);
  }
  /** Deliver a resize independently of whether teardown has already occurred. */
  deliver(): void {
    this.callback([], this);
  }
}

beforeEach(() => {
  observers.length = 0;
  frames.clear();
  Object.defineProperty(window, 'ResizeObserver', { configurable: true, value: ResizeHarness });
  let next = 0;
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.set(++next, callback);
    return next;
  });
  jest.spyOn(window, 'cancelAnimationFrame').mockImplementation((id) => {
    frames.delete(id);
  });
});

afterEach(() => {
  if (descriptor) Object.defineProperty(window, 'ResizeObserver', descriptor);
  else Reflect.deleteProperty(window, 'ResizeObserver');
  jest.restoreAllMocks();
});

/** Flush one accepted geometry delivery without elapsed-time waiting. */
function flush(): void {
  const pending = [...frames.values()];
  frames.clear();
  for (const callback of pending) callback(0);
}

/** Build a strip with independently controlled browser geometry. */
function harness(initialWidth = 803) {
  const frame = document.createElement('header');
  const viewport = document.createElement('div');
  const tabs = document.createElement('div');
  const controls = {
    overflow: document.createElement('button'),
    previous: document.createElement('button'),
    next: document.createElement('button'),
  };
  let width = initialWidth;
  Object.defineProperty(frame, 'clientWidth', { get: () => width });
  Object.defineProperty(viewport, 'clientWidth', {
    get: () => Number.parseFloat(viewport.style.width),
  });
  Object.defineProperty(viewport, 'scrollWidth', {
    get: () => Number.parseFloat(tabs.style.getPropertyValue('--sc-workflow-tabs-width')),
  });
  const resized = jest.fn();
  const close = jest.fn();
  const layout = new WorkflowTabStripLayout(
    document,
    frame,
    viewport,
    tabs,
    controls,
    resized,
    close,
    jest.fn(),
  );
  return {
    frame,
    viewport,
    tabs,
    controls,
    resized,
    close,
    layout,
    resize: (next: number) => {
      width = next;
      observers[0]?.deliver();
      flush();
    },
  };
}

test('tabs condense from 180 to 120 pixels before scrolling or exposing overflow controls', () => {
  const h = harness();
  expect(observers[0]?.targets).toEqual(new Set([h.frame]));
  h.layout.update(4);
  expect(h.viewport.style.width).toBe('720px');
  expect(h.resized).toHaveBeenLastCalledWith(180);
  expect(h.controls.overflow.hidden).toBe(true);
  h.resize(683);
  expect(h.viewport.style.width).toBe('600px');
  expect(h.resized).toHaveBeenLastCalledWith(150);
  h.resize(563);
  expect(h.resized).toHaveBeenLastCalledWith(120);
  expect(h.controls.overflow.hidden).toBe(true);
  h.resize(562);
  expect(h.controls.overflow.hidden).toBe(false);
  expect(h.controls.previous.hidden).toBe(true);
  expect(h.controls.next.hidden).toBe(false);
  expect(h.viewport.style.width).toBe('415px');
  h.resize(562);
  expect(h.controls.overflow.hidden).toBe(false);
  h.layout.update(3);
  expect(h.controls.overflow.hidden).toBe(true);
  expect(h.close).toHaveBeenCalledTimes(1);
  h.layout.update(0);
  expect(h.viewport.style.width).toBe('0px');
  expect(h.tabs.style.getPropertyValue('--sc-workflow-tabs-width')).toBe('0px');
  h.resize(0);
  expect(h.controls.overflow.hidden).toBe(true);
  h.layout.dispose();
});

test('arrow visibility follows endpoints and returns its space to the scrolling viewport', () => {
  const h = harness(562);
  h.layout.update(4);
  expect(h.controls.previous.hidden).toBe(true);
  expect(h.controls.next.hidden).toBe(false);
  h.controls.next.click();
  expect(h.viewport.scrollLeft).toBe(65);
  expect(h.controls.next.hidden).toBe(true);
  expect(h.controls.previous.hidden).toBe(false);
  expect(h.viewport.style.width).toBe('415px');
  h.controls.previous.click();
  expect(h.viewport.scrollLeft).toBe(0);
  h.viewport.scrollLeft = 50;
  h.viewport.dispatchEvent(new Event('scroll'));
  expect(h.controls.previous.hidden).toBe(false);
  expect(h.controls.next.hidden).toBe(false);
  expect(h.viewport.style.width).toBe('383px');
  h.viewport.scrollLeft = 97;
  h.viewport.dispatchEvent(new Event('scroll'));
  expect(h.controls.next.hidden).toBe(true);
  expect(h.viewport.scrollLeft).toBe(65);
  h.viewport.dispatchEvent(new Event('scroll'));
  expect(h.controls.next.hidden).toBe(true);
  h.viewport.scrollLeft = 50;
  h.viewport.dispatchEvent(new Event('scroll'));
  h.layout.dispose();
  h.controls.next.click();
  expect(h.viewport.scrollLeft).toBe(50);
});

test('teardown cancels queued geometry and rejects late observer deliveries or collection updates', () => {
  const h = harness(900);
  h.layout.update(2);
  observers[0]?.deliver();
  observers[0]?.deliver();
  expect(frames.size).toBe(1);
  h.layout.dispose();
  expect(observers[0]?.disconnect).toHaveBeenCalledTimes(1);
  expect(frames.size).toBe(0);
  observers[0]?.deliver();
  h.layout.update(9);
  flush();
  expect(frames.size).toBe(0);
  expect(h.resized).toHaveBeenCalledTimes(1);
  expect(h.viewport.style.width).toBe('360px');
});

test('generation strip and native caption reservation participate in condensation and overflow', () => {
  const h = harness(1127);
  h.frame.style.paddingRight = '324px';
  h.layout.update(4);
  expect(h.resized).toHaveBeenLastCalledWith(180);
  h.frame.style.paddingRight = '504px';
  h.resize(1127);
  expect(h.resized).toHaveBeenLastCalledWith(135);
  expect(h.controls.overflow.hidden).toBe(true);
  h.frame.style.paddingRight = '565px';
  h.resize(1127);
  expect(h.controls.overflow.hidden).toBe(false);
  h.layout.dispose();
});

test('layout rejects a detached document without a browser geometry lifetime', () => {
  const h = harness();
  expect(
    () =>
      new WorkflowTabStripLayout(
        document.implementation.createHTMLDocument(),
        h.frame,
        h.viewport,
        h.tabs,
        h.controls,
        jest.fn(),
        jest.fn(),
        jest.fn(),
      ),
  ).toThrow('browser window');
  h.layout.dispose();
});
