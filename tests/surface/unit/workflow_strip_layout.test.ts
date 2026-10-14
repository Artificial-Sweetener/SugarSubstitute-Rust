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

test('container resizing and removal cross the exact-fit boundary without overflow feedback', () => {
  const frame = document.createElement('header');
  const viewport = document.createElement('div');
  const tabs = document.createElement('div');
  const overflow = document.createElement('button');
  let width = 808;
  Object.defineProperty(frame, 'clientWidth', { get: () => width });
  const resized = jest.fn();
  const close = jest.fn();
  const layout = new WorkflowTabStripLayout(
    document,
    frame,
    viewport,
    tabs,
    overflow,
    resized,
    close,
  );
  expect(observers[0]?.targets).toEqual(new Set([frame]));
  layout.update(4);
  expect(viewport.style.width).toBe('760px');
  expect(overflow.hidden).toBe(true);
  layout.update(4);
  expect(resized).toHaveBeenCalledTimes(1);
  width = 807;
  observers[0]?.deliver();
  observers[0]?.deliver();
  expect(frames.size).toBe(1);
  flush();
  expect(overflow.hidden).toBe(false);
  observers[0]?.deliver();
  flush();
  expect(overflow.hidden).toBe(false);
  expect(close).not.toHaveBeenCalled();
  layout.update(3);
  expect(overflow.hidden).toBe(true);
  expect(close).toHaveBeenCalledTimes(1);
  layout.update(0);
  expect(viewport.style.width).toBe('40px');
  expect(tabs.style.getPropertyValue('--sc-workflow-tabs-width')).toBe('0px');
  expect(overflow.hidden).toBe(true);
  width = 0;
  observers[0]?.deliver();
  flush();
  expect(overflow.hidden).toBe(true);
  layout.dispose();
});

test('teardown cancels queued geometry and rejects late observer deliveries or collection updates', () => {
  const frame = document.createElement('header');
  Object.defineProperty(frame, 'clientWidth', { value: 900 });
  const viewport = document.createElement('div');
  const resized = jest.fn();
  const layout = new WorkflowTabStripLayout(
    document,
    frame,
    viewport,
    document.createElement('div'),
    document.createElement('button'),
    resized,
    jest.fn(),
  );
  layout.update(2);
  observers[0]?.deliver();
  expect(frames.size).toBe(1);
  layout.dispose();
  expect(observers[0]?.disconnect).toHaveBeenCalledTimes(1);
  expect(frames.size).toBe(0);
  observers[0]?.deliver();
  layout.update(9);
  flush();
  expect(frames.size).toBe(0);
  expect(resized).toHaveBeenCalledTimes(1);
  expect(viewport.style.width).toBe('400px');
});

test('layout rejects a detached document without a browser geometry lifetime', () => {
  expect(
    () =>
      new WorkflowTabStripLayout(
        document.implementation.createHTMLDocument(),
        document.createElement('header'),
        document.createElement('div'),
        document.createElement('div'),
        document.createElement('button'),
        jest.fn(),
        jest.fn(),
      ),
  ).toThrow('browser window');
});
