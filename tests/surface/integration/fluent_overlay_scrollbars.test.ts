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
/** Prove overlay scrolling, dynamic discovery, and reversible ownership without layout wrappers. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { FluentOverlayScrollbarHost } from '../../../frontend/comfyui/ui/surface/appearance/FluentOverlayScrollbarHost.js';

afterEach(() => {
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Provide deterministic browser geometry while keeping real native scroll properties. */
function scrollArea(): HTMLDivElement {
  const area = document.createElement('div');
  area.style.overflow = 'auto';
  Object.defineProperties(area, {
    clientWidth: { value: 300 },
    offsetWidth: { value: 300 },
    scrollWidth: { value: 600, configurable: true },
    clientHeight: { value: 200 },
    offsetHeight: { value: 200 },
    scrollHeight: { value: 1000, configurable: true },
  });
  jest.spyOn(area, 'getBoundingClientRect').mockReturnValue({
    x: 20,
    y: 20,
    left: 20,
    top: 20,
    right: 320,
    bottom: 220,
    width: 300,
    height: 200,
    toJSON: () => ({}),
  });
  return area;
}

/** Flush observer delivery before the coalesced animation-frame update. */
async function flush(frames: FrameRequestCallback[]): Promise<void> {
  await Promise.resolve();
  frames.splice(0).forEach((frame) => frame(0));
  await Promise.resolve();
}

test('portal mounts and local input do not rediscover or measure an unrelated scroll owner', async () => {
  const area = scrollArea(),
    other = scrollArea();
  const input = document.createElement('input');
  area.append(input);
  document.body.append(area, other);
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((frame) => {
    frames.push(frame);
    return frames.length;
  });
  const host = new FluentOverlayScrollbarHost(document, jest.fn());
  host.enable();
  await flush(frames);
  await flush(frames);
  const bounds = jest.mocked(other.getBoundingClientRect);
  bounds.mockClear();
  const styles = jest.spyOn(window, 'getComputedStyle');
  const popup = document.createElement('div');
  popup.setAttribute('role', 'listbox');
  document.body.append(popup);
  await flush(frames);
  await flush(frames);
  input.dispatchEvent(new Event('input', { bubbles: true }));
  await flush(frames);
  expect(bounds).not.toHaveBeenCalled();
  expect(styles.mock.calls.some(([element]) => element === other)).toBe(false);
  host.clear();
});

test('handles both axes through native state and restores untouched content on disable', async () => {
  const area = scrollArea();
  const content = document.createElement('input');
  content.value = '<img src=x>';
  area.append(content);
  document.body.append(area);
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((frame) => {
    frames.push(frame);
    return frames.length;
  });
  const errors = jest.fn();
  const host = new FluentOverlayScrollbarHost(document, errors);
  host.enable();
  await flush(frames);
  const vertical = document.querySelector<HTMLElement>(
    '.sc-fluent-scrollbar[aria-orientation="vertical"]',
  );
  const horizontal = document.querySelector<HTMLElement>(
    '.sc-fluent-scrollbar[aria-orientation="horizontal"]',
  );
  expect(vertical?.hidden).toBe(false);
  expect(horizontal?.hidden).toBe(false);
  expect(vertical?.getAttribute('aria-controls')).toBe(area.id);
  expect(area.children).toHaveLength(1);
  expect(area.style.position).toBe('');
  expect(area.style.overflow).toBe('auto');
  vertical?.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'End', bubbles: true, cancelable: true }),
  );
  await flush(frames);
  expect(area.scrollTop).toBe(800);
  expect(vertical?.getAttribute('aria-valuenow')).toBe('800');
  horizontal?.dispatchEvent(
    new WheelEvent('wheel', { deltaY: 80, bubbles: true, cancelable: true }),
  );
  await flush(frames);
  expect(area.scrollLeft).toBe(80);
  Object.defineProperty(area, 'scrollHeight', { value: 200, configurable: true });
  area.dispatchEvent(new Event('scroll'));
  await flush(frames);
  expect(vertical?.hidden).toBe(true);
  host.clear();
  expect(document.querySelector('.sc-fluent-scrollbar-layer')).toBeNull();
  expect(area.id).toBe('');
  expect(area.scrollTop).toBe(800);
  expect(content.value).toBe('<img src=x>');
  expect(errors).not.toHaveBeenCalled();
});

test('discovers asynchronous shadow content and removes only policy-owned shadow styles', async () => {
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((frame) => {
    frames.push(frame);
    return frames.length;
  });
  const errors = jest.fn();
  const host = new FluentOverlayScrollbarHost(document, errors);
  host.enable();
  await flush(frames);
  const custom = document.createElement('div');
  const shadow = custom.attachShadow({ mode: 'open' });
  const style = document.createElement('style');
  style.textContent = ':host {color: pink}';
  const area = scrollArea();
  area.id = 'existing-scroll-owner';
  shadow.append(style, area);
  document.body.append(custom);
  await flush(frames);
  expect(shadow.querySelector('[data-sc-scrollbar-policy]')?.textContent).toContain(
    'scrollbar-width:none',
  );
  expect(document.querySelector('[aria-controls="existing-scroll-owner"]')).not.toBeNull();
  custom.remove();
  await flush(frames);
  expect(document.querySelector('[aria-controls="existing-scroll-owner"]')).toBeNull();
  host.clear();
  expect(shadow.querySelector('[data-sc-scrollbar-policy]')).toBeNull();
  expect(style.textContent).toBe(':host {color: pink}');
  expect(area.id).toBe('existing-scroll-owner');
  expect(errors).not.toHaveBeenCalled();
});

test('captures thumb dragging without bubbling into graph interaction', async () => {
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((frame) => {
    frames.push(frame);
    return frames.length;
  });
  const area = scrollArea();
  document.body.append(area);
  const host = new FluentOverlayScrollbarHost(document, jest.fn());
  host.enable();
  await flush(frames);
  const thumb = document.querySelector<HTMLElement>(
    '.sc-fluent-scrollbar[aria-orientation="vertical"]',
  );
  if (!thumb) throw new Error('Missing vertical thumb');
  Object.assign(thumb, {
    setPointerCapture: jest.fn(),
    hasPointerCapture: () => true,
    releasePointerCapture: jest.fn(),
  });
  const graphDrag = jest.fn();
  document.addEventListener('pointerdown', graphDrag, true);
  thumb.dispatchEvent(
    Object.assign(new Event('pointerdown', { bubbles: true, cancelable: true }), {
      button: 0,
      pointerId: 1,
      clientY: 30,
      clientX: 310,
    }),
  );
  thumb.dispatchEvent(
    Object.assign(new Event('pointermove', { bubbles: true, cancelable: true }), {
      pointerId: 1,
      clientY: 109.2,
      clientX: 310,
    }),
  );
  await flush(frames);
  expect(area.scrollTop).toBeCloseTo(400);
  expect(graphDrag).not.toHaveBeenCalled();
  thumb.dispatchEvent(new Event('pointerup'));
  expect(thumb.hasAttribute('data-dragging')).toBe(false);
  document.body.dispatchEvent(
    Object.assign(new Event('pointerdown', { bubbles: true }), { button: 0 }),
  );
  expect(graphDrag).toHaveBeenCalledTimes(1);
  document.removeEventListener('pointerdown', graphDrag, true);
  host.clear();
});
