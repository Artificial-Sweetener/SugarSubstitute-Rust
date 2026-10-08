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
/** Prove automatic wheel routing across native horizontal surfaces, themes, and nested controls. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { ScrollSurfaceWheelHost } from '../../../frontend/comfyui/ui/surface/ScrollSurfaceWheelHost.js';

const hosts: ScrollSurfaceWheelHost[] = [];

afterEach(() => {
  for (const host of hosts.splice(0)) host.dispose();
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Model native overflow without depending on a particular feature's classes or markup. */
function surface(vertical = false): HTMLDivElement {
  const area = document.createElement('div');
  area.style.overflow = 'auto';
  Object.defineProperties(area, {
    clientWidth: { value: 200 },
    scrollWidth: { value: 700, configurable: true },
    clientHeight: { value: 40 },
    scrollHeight: { value: vertical ? 200 : 40 },
  });
  return area;
}

/** Install the document-wide owner before mounting test surfaces. */
function setup(): ScrollSurfaceWheelHost {
  const host = new ScrollSurfaceWheelHost(document);
  hosts.push(host);
  host.setup();
  return host;
}

/** Dispatch real DOM wheel propagation through the selected native descendant. */
function wheel(target: HTMLElement, options: WheelEventInit): WheelEvent {
  const event = new WheelEvent('wheel', {
    bubbles: true,
    composed: true,
    cancelable: true,
    ...options,
  });
  target.dispatchEvent(event);
  return event;
}

test.each<[WheelEventInit, number]>([
  [{ deltaY: 70 }, 70],
  [{ deltaX: 80 }, 80],
  [{ deltaX: 30, deltaY: 90 }, 30],
  [{ deltaY: 25, shiftKey: true }, 25],
  [{ deltaY: 2, deltaMode: 1 }, 32],
  [{ deltaY: 1, deltaMode: 2 }, 200],
])('routes gesture %j across the entire horizontal surface', (options, position) => {
  setup();
  const area = surface();
  const control = document.createElement('input');
  control.type = 'number';
  control.value = '17';
  area.append(control);
  document.body.append(area);
  const widgetWheel = jest.fn();
  control.addEventListener('wheel', widgetWheel);
  const event = wheel(control, options);
  expect(area.scrollLeft).toBe(position);
  expect(event.defaultPrevented).toBe(true);
  expect(widgetWheel).not.toHaveBeenCalled();
  expect(control.value).toBe('17');
});

test('keeps boundary gestures inside the horizontal surface and clamps both ends', () => {
  setup();
  const area = surface();
  document.body.append(area);
  const graphWheel = jest.fn();
  document.addEventListener('wheel', graphWheel);
  expect(wheel(area, { deltaY: -100 }).defaultPrevented).toBe(true);
  expect(area.scrollLeft).toBe(0);
  wheel(area, { deltaY: 1000 });
  expect(area.scrollLeft).toBe(500);
  expect(wheel(area, { deltaY: 100 }).defaultPrevented).toBe(true);
  expect(graphWheel).not.toHaveBeenCalled();
  document.removeEventListener('wheel', graphWheel);
});

test('leaves scrollbar gestures with their own owner rather than a containing scroll surface', () => {
  setup();
  const area = surface();
  const thumb = document.createElement('div');
  thumb.setAttribute('role', 'scrollbar');
  area.append(thumb);
  document.body.append(area);
  const ownWheel = jest.fn();
  thumb.addEventListener('wheel', ownWheel);
  expect(wheel(thumb, { deltaY: 50 }).defaultPrevented).toBe(false);
  expect(ownWheel).toHaveBeenCalledTimes(1);
  expect(area.scrollLeft).toBe(0);
});

test('preserves native vertical input in a two-axis surface while handling horizontal intent', () => {
  setup();
  const area = surface(true);
  document.body.append(area);
  expect(wheel(area, { deltaY: 50 }).defaultPrevented).toBe(false);
  expect(wheel(area, { deltaY: 50, deltaX: 20 }).defaultPrevented).toBe(false);
  expect(area.scrollLeft).toBe(0);
  wheel(area, { deltaX: 50 });
  wheel(area, { deltaY: 20, shiftKey: true });
  expect(area.scrollLeft).toBe(70);
});

test('retains nested vertical scrolling and sends shifted gestures to the containing horizontal strip', () => {
  setup();
  const outer = surface();
  const inner = surface(true);
  inner.style.overflowX = 'hidden';
  outer.append(inner);
  document.body.append(outer);
  expect(wheel(inner, { deltaY: 40 }).defaultPrevented).toBe(false);
  expect(outer.scrollLeft).toBe(0);
  wheel(inner, { deltaY: 40, shiftKey: true });
  expect(outer.scrollLeft).toBe(40);
});

test('routes open shadow content and preserves RTL physical and logical directions', () => {
  setup();
  const custom = document.createElement('div');
  const shadow = custom.attachShadow({ mode: 'open' });
  const area = surface();
  area.style.direction = 'rtl';
  const control = document.createElement('button');
  area.append(control);
  shadow.append(area);
  document.body.append(custom);
  wheel(control, { deltaY: 80 });
  expect(area.scrollLeft).toBe(-80);
  wheel(control, { deltaX: 30 });
  expect(area.scrollLeft).toBe(-50);
  wheel(control, { deltaY: -100 });
  expect(area.scrollLeft).toBe(0);
});

test('leaves zoom, fitting content and clipped containers untouched', () => {
  setup();
  const area = surface();
  document.body.append(area);
  expect(wheel(area, { deltaY: 30, ctrlKey: true }).defaultPrevented).toBe(false);
  expect(wheel(area, { deltaY: 30, metaKey: true }).defaultPrevented).toBe(false);
  area.style.overflow = 'hidden';
  expect(wheel(area, { deltaY: 30 }).defaultPrevented).toBe(false);
  area.style.overflow = 'auto';
  Object.defineProperty(area, 'scrollWidth', { value: 200 });
  expect(wheel(area, { deltaY: 30 }).defaultPrevented).toBe(false);
  expect(area.scrollLeft).toBe(0);
});

test('handles newly mounted surfaces without observers, subtree scans or unrelated geometry reads', () => {
  const host = setup();
  host.setup();
  const area = surface(),
    unrelated = surface();
  document.body.append(area, unrelated);
  const query = jest.spyOn(document, 'querySelectorAll');
  const styles = jest.spyOn(window, 'getComputedStyle');
  wheel(area, { deltaY: 10 });
  expect(area.scrollLeft).toBe(10);
  expect(query).not.toHaveBeenCalled();
  expect(styles.mock.calls.every(([element]) => element === area)).toBe(true);
  host.dispose();
  expect(wheel(area, { deltaY: 10 }).defaultPrevented).toBe(false);
  expect(area.scrollLeft).toBe(10);
  host.setup();
  window.dispatchEvent(new Event('pagehide'));
  expect(wheel(area, { deltaY: 10 }).defaultPrevented).toBe(false);
});
