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
import { setSurfaceGeometryMoving } from '../../../frontend/comfyui/ui/surface/motion/SurfaceGeometryMotion.js';
import { fluentScrollbarPresentationCss } from '../../../frontend/comfyui/ui/surface/appearance/FluentScrollbarStyles.js';

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

test('keeps the same viewport thumb visible when its positioned scroll owner moves the layer beyond the viewport', async () => {
  const area = scrollArea();
  area.style.position = 'relative';
  document.body.append(area);
  const style = document.createElement('style');
  style.textContent = fluentScrollbarPresentationCss();
  document.head.append(style);
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((frame) => {
    frames.push(frame);
    return frames.length;
  });
  const host = new FluentOverlayScrollbarHost(document, jest.fn());
  try {
    host.enable();
    await flush(frames);
    const layer = area.querySelector<HTMLElement>('.sc-fluent-scrollbar-layer')!;
    const thumb = layer.querySelector<HTMLElement>('[aria-orientation="vertical"]')!;
    jest.spyOn(layer, 'getBoundingClientRect').mockImplementation(() => ({
      x: 20,
      y: 20 - area.scrollTop,
      left: 20,
      top: 20 - area.scrollTop,
      right: 320,
      bottom: 220 - area.scrollTop,
      width: 300,
      height: 200,
      toJSON: () => ({}),
    }));
    for (const position of [400, 600, 800, 0]) {
      area.scrollTop = position;
      area.dispatchEvent(new Event('scroll', { bubbles: true }));
      await flush(frames);
      expect(layer.querySelector('[aria-orientation="vertical"]')).toBe(thumb);
      expect(thumb.hidden).toBe(false);
      const top = layer.getBoundingClientRect().top + Number.parseFloat(thumb.style.top);
      expect(top).toBeGreaterThanOrEqual(20);
      expect(top + Number.parseFloat(thumb.style.height)).toBeLessThanOrEqual(220);
      expect(getComputedStyle(layer).overflow).toBe('visible');
    }
  } finally {
    host.clear();
    style.remove();
  }
});

test('reads new overflow candidates before mounting any tracks', async () => {
  const first = scrollArea(),
    second = scrollArea();
  document.body.append(first, second);
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((frame) => {
    frames.push(frame);
    return frames.length;
  });
  const styles = window.getComputedStyle.bind(window);
  let tracksWhenSecondRead: number | null = null;
  jest.spyOn(window, 'getComputedStyle').mockImplementation((element, pseudo) => {
    if (element === second && tracksWhenSecondRead === null)
      tracksWhenSecondRead = document.querySelectorAll('.sc-fluent-scrollbar').length;
    return styles(element, pseudo);
  });
  const host = new FluentOverlayScrollbarHost(document, jest.fn());
  host.enable();
  await flush(frames);
  expect(tracksWhenSecondRead).toBe(0);
  expect(document.querySelectorAll('.sc-fluent-scrollbar')).toHaveLength(4);
  host.clear();
});

test('places an opted-in horizontal handle below content without changing layout or vertical geometry', async () => {
  const area = scrollArea();
  area.dataset.scScrollbarHorizontalOutset = '4';
  const surface = document.createElement('div');
  surface.style.position = 'relative';
  surface.style.overflowY = 'hidden';
  jest.spyOn(surface, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    right: 400,
    bottom: 222,
    width: 400,
    height: 222,
    toJSON: () => ({}),
  });
  surface.append(area);
  document.body.append(surface);
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((frame) => {
    frames.push(frame);
    return frames.length;
  });
  const host = new FluentOverlayScrollbarHost(document, jest.fn());
  host.enable();
  await flush(frames);
  const horizontal = surface.querySelector<HTMLElement>('[aria-orientation="horizontal"]');
  const vertical = surface.querySelector<HTMLElement>('[aria-orientation="vertical"]');
  expect(horizontal?.style.top).toBe('210px');
  expect(horizontal?.style.height).toBe('12px');
  expect(vertical?.style.top).toBe('21px');
  expect(vertical?.style.height).toBe('39.6px');
  expect(area.clientWidth).toBe(300);
  expect(area.clientHeight).toBe(200);
  expect(area.style.padding).toBe('');
  expect(surface.querySelector<HTMLElement>('.sc-fluent-scrollbar-layer')?.style.bottom).toBe(
    '-4px',
  );
  host.clear();
  expect(surface.querySelector('.sc-fluent-scrollbar-layer')).toBeNull();
});

test('honors textarea overflow policy and removes handles when text fits', async () => {
  const textarea = document.createElement('textarea');
  textarea.style.overflowY = 'hidden';
  textarea.style.overflowX = 'hidden';
  Object.defineProperties(textarea, {
    clientWidth: { value: 300 },
    offsetWidth: { value: 300 },
    scrollWidth: { value: 300 },
    clientHeight: { value: 50 },
    offsetHeight: { value: 52 },
    scrollHeight: { value: 52, configurable: true },
  });
  jest.spyOn(textarea, 'getBoundingClientRect').mockReturnValue({
    x: 20,
    y: 20,
    left: 20,
    top: 20,
    right: 320,
    bottom: 72,
    width: 300,
    height: 52,
    toJSON: () => ({}),
  });
  document.body.append(textarea);
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((frame) => {
    frames.push(frame);
    return frames.length;
  });
  const host = new FluentOverlayScrollbarHost(document, jest.fn());
  host.enable();
  await flush(frames);
  const thumb = document.querySelector<HTMLElement>('[aria-orientation="vertical"]');
  expect(thumb?.hidden).toBe(true);
  textarea.style.overflowY = 'auto';
  Object.defineProperty(textarea, 'scrollHeight', { value: 200, configurable: true });
  await flush(frames);
  expect(thumb?.hidden).toBe(false);
  expect(thumb?.getAttribute('aria-valuemax')).toBe('150');
  Object.defineProperty(textarea, 'scrollHeight', { value: 50, configurable: true });
  textarea.dispatchEvent(new Event('input', { bubbles: true }));
  await flush(frames);
  expect(thumb?.hidden).toBe(true);
  host.clear();
});

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

test('keeps tracks inside positioned owners and reparents them when the owning surface changes', async () => {
  const area = scrollArea();
  const surface = document.createElement('div');
  surface.style.position = 'fixed';
  surface.style.zIndex = '12000';
  surface.append(area);
  document.body.append(surface);
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((frame) => {
    frames.push(frame);
    return frames.length;
  });
  const host = new FluentOverlayScrollbarHost(document, jest.fn());
  host.enable();
  await flush(frames);
  const thumb = document.querySelector<HTMLElement>('[aria-orientation="vertical"]');
  expect(thumb?.closest('.sc-fluent-scrollbar-layer')?.parentElement).toBe(surface);
  const replacement = document.createElement('div');
  replacement.style.position = 'relative';
  document.body.append(replacement);
  replacement.append(area);
  await flush(frames);
  expect(thumb?.closest('.sc-fluent-scrollbar-layer')?.parentElement).toBe(replacement);
  expect(surface.querySelector('.sc-fluent-scrollbar-layer')).toBeNull();
  expect(document.querySelector('[aria-orientation="vertical"]')).toBe(thumb);
  host.clear();
  expect(replacement.querySelector('.sc-fluent-scrollbar-layer')).toBeNull();
});

test('defers moving-region geometry until settlement while unrelated native scrolling remains responsive', async () => {
  const area = scrollArea(),
    other = scrollArea();
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
  const bounds = jest.mocked(area.getBoundingClientRect);
  bounds.mockClear();
  const unrelated = jest.mocked(other.getBoundingClientRect);
  unrelated.mockClear();
  setSurfaceGeometryMoving(area, true);
  for (let index = 0; index < 3; index++) {
    area.style.width = `${300 + index}px`;
    area.dispatchEvent(new Event('scroll', { bubbles: true }));
    await flush(frames);
  }
  expect(bounds).not.toHaveBeenCalled();
  other.dispatchEvent(new Event('scroll', { bubbles: true }));
  await flush(frames);
  expect(unrelated).toHaveBeenCalled();
  setSurfaceGeometryMoving(area, false);
  await flush(frames);
  expect(bounds).toHaveBeenCalledTimes(1);
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

test('keeps horizontal handles outside content flow without changing owner or widget layout', async () => {
  const area = scrollArea();
  area.style.padding = '3px 7px';
  const widget = document.createElement('button');
  widget.textContent = 'Native widget';
  area.append(widget);
  const surface = document.createElement('div');
  surface.style.position = 'relative';
  surface.append(area);
  document.body.append(surface);
  const style = area.getAttribute('style');
  const dimensions = [area.clientWidth, area.clientHeight, area.offsetWidth, area.offsetHeight];
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((frame) => {
    frames.push(frame);
    return frames.length;
  });
  const host = new FluentOverlayScrollbarHost(document, jest.fn());
  host.enable();
  await flush(frames);
  const thumb = surface.querySelector<HTMLElement>('[aria-orientation="horizontal"]');
  expect(thumb?.hidden).toBe(false);
  expect(thumb?.closest('.sc-fluent-scrollbar-layer')?.parentElement).toBe(surface);
  expect(area.children).toHaveLength(1);
  expect(area.firstElementChild).toBe(widget);
  expect(area.getAttribute('style')).toBe(style);
  expect(area.hasAttribute('data-sc-horizontal-scrollbar')).toBe(false);
  expect([area.clientWidth, area.clientHeight, area.offsetWidth, area.offsetHeight]).toEqual(
    dimensions,
  );
  host.clear();
  expect(area.getAttribute('style')).toBe(style);
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
  expect(shadow.querySelector('[aria-controls="existing-scroll-owner"]')).not.toBeNull();
  custom.remove();
  await flush(frames);
  expect(shadow.querySelector('[aria-controls="existing-scroll-owner"]')).toBeNull();
  host.clear();
  expect(shadow.querySelector('[data-sc-scrollbar-policy]')).toBeNull();
  expect(style.textContent).toBe(':host {color: pink}');
  expect(area.id).toBe('existing-scroll-owner');
  expect(errors).not.toHaveBeenCalled();
});

test.each([false, true])(
  'captures thumb dragging without graph or popup handlers in shadow mode %s',
  async (shadowMode) => {
    const frames: FrameRequestCallback[] = [];
    jest.spyOn(window, 'requestAnimationFrame').mockImplementation((frame) => {
      frames.push(frame);
      return frames.length;
    });
    const area = scrollArea();
    const surface = document.createElement('div');
    const root = shadowMode ? surface.attachShadow({ mode: 'open' }) : surface;
    root.append(area);
    document.body.append(surface);
    const host = new FluentOverlayScrollbarHost(document, jest.fn());
    host.enable();
    await flush(frames);
    const thumb = (shadowMode ? root : document).querySelector<HTMLElement>(
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
      Object.assign(new Event('pointerdown', { bubbles: true, cancelable: true, composed: true }), {
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
  },
);

test('workflow tab navigation remains handle-free through appearance and renderer lifetimes', async () => {
  const area = scrollArea();
  area.id = 'sc-workflow-strip';
  area.setAttribute('data-sc-scrollbar-managed', '');
  const frame = document.createElement('header');
  frame.style.position = 'relative';
  frame.append(area);
  document.body.append(frame);
  const frames: FrameRequestCallback[] = [];
  jest.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
    frames.push(callback);
    return frames.length;
  });
  const global = new FluentOverlayScrollbarHost(document, jest.fn());
  global.enable();
  await flush(frames);
  expect(document.querySelectorAll(`[aria-controls="${area.id}"]`)).toHaveLength(0);
  global.clear();
  global.enable();
  await flush(frames);
  expect(document.querySelectorAll(`[aria-controls="${area.id}"]`)).toHaveLength(0);
  expect(area.hasAttribute('data-sc-scrollbar-managed')).toBe(true);
  global.clear();
});
