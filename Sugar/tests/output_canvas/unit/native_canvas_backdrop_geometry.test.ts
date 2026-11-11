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
/** Measure actual material regions and their independent, bounded observer lifetime. */
import { jest } from '@jest/globals';
import { NativeCanvasBackdrops } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/NativeCanvasBackdrops.js';
import { CanvasSurfaceGeometryObserver } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/CanvasSurfaceGeometryObserver.js';
import { CanvasFrameFixture } from '../support/CanvasFrameFixture.js';

/** Own only external resize deliveries; the production observer performs every measurement. */
class ResizeBoundary implements ResizeObserver {
  static instances: ResizeBoundary[] = [];
  readonly elements = new Set<Element>();
  /** Retain the browser boundary's actual callback for controlled delivery. */
  constructor(readonly callback: ResizeObserverCallback) {
    ResizeBoundary.instances.push(this);
  }
  /** Record only explicitly observed elements. */
  observe(element: Element): void {
    this.elements.add(element);
  }
  /** Release one retired element. */
  unobserve(element: Element): void {
    this.elements.delete(element);
  }
  /** Release all elements at the observer lifetime boundary. */
  disconnect(): void {
    this.elements.clear();
  }
  /** Deliver one explicit external resize notification. */
  deliver(): void {
    this.callback([], this);
  }
}

const previousResize = window.ResizeObserver;
beforeEach(() => {
  ResizeBoundary.instances = [];
  window.ResizeObserver = ResizeBoundary;
});
afterEach(() => {
  window.ResizeObserver = previousResize;
  jest.restoreAllMocks();
  document.body.replaceChildren();
});

/** Supply the browser rectangle boundary without requiring a JSDOM constructor. */
function rect(left: number, top: number, width: number, height: number): DOMRect {
  return {
    x: left,
    y: top,
    left,
    top,
    width,
    height,
    right: left + width,
    bottom: top + height,
    toJSON: () => ({}),
  };
}

/** Supply browser-owned allocation while keeping all material arithmetic in the actual owner. */
function fixture() {
  const root = document.createElement('section');
  const viewport = document.createElement('div');
  viewport.dataset.scCanvasGeometry = '';
  root.append(viewport);
  document.body.append(root);
  jest.spyOn(viewport, 'getBoundingClientRect').mockReturnValue(rect(10, 20, 500, 400));
  const bar = document.createElement('div');
  bar.dataset.scCanvasBackdrop = 'navigation';
  bar.style.borderRadius = '8px';
  bar.style.setProperty('--p-toast-blur', '12px');
  root.append(bar);
  const bounds = jest.spyOn(bar, 'getBoundingClientRect').mockReturnValue(rect(18, 370, 200, 42));
  const invalidated = jest.fn();
  const backdrops = new NativeCanvasBackdrops(root, invalidated);
  return { root, viewport, bar, bounds, invalidated, backdrops };
}

test('publishes backdrop geometry independently of native input routing and density', () => {
  const f = fixture();
  const observer = new CanvasSurfaceGeometryObserver(
    f.viewport,
    new CanvasFrameFixture(),
    () => {},
    null,
    f.backdrops,
  );
  try {
    const geometry = observer.read();
    expect(geometry.backdrops).toEqual([
      { x: 8, y: 350, width: 200, height: 42, cornerRadius: 8, sigma: 12 },
    ]);
    expect(geometry.occlusions).toBeUndefined();
    expect(geometry.modal).toBeUndefined();
    expect(Object.isFrozen(geometry.backdrops)).toBe(true);
    expect(Object.isFrozen(geometry.backdrops?.[0])).toBe(true);
    expect(f.backdrops.current).toBe(true);
    const reads = f.bounds.mock.calls.length;
    for (let camera = 0; camera < 30; camera++) expect(f.backdrops.current).toBe(true);
    expect(f.bounds.mock.calls.length).toBe(reads);
  } finally {
    observer.dispose();
  }
});

test('size invalidation withdraws transparency immediately, coalesces measurement, and retires callbacks', () => {
  const f = fixture();
  const frames = new CanvasFrameFixture();
  const changed = jest.fn();
  const observer = new CanvasSurfaceGeometryObserver(
    f.viewport,
    frames,
    changed,
    null,
    f.backdrops,
  );
  observer.read();
  const resize = ResizeBoundary.instances.find((boundary) => boundary.elements.has(f.bar))!;
  try {
    f.bounds.mockReturnValue(rect(18, 370, 320, 42));
    resize.deliver();
    resize.deliver();
    expect(f.backdrops.current).toBe(false);
    expect(f.invalidated).toHaveBeenCalledTimes(1);
    expect(frames.pending).toBe(1);
    frames.advance(16);
    expect(changed.mock.calls[0]?.[0]).toMatchObject({ backdrops: [{ width: 320 }] });
    expect(f.backdrops.current).toBe(true);
    resize.deliver();
    observer.dispose();
    resize.deliver();
    frames.advance(16);
    expect(frames.pending).toBe(0);
    expect(changed).toHaveBeenCalledTimes(1);
    expect(resize.elements.size).toBe(0);
    expect(f.backdrops.current).toBe(false);
  } finally {
    observer.dispose();
  }
});

test('unrelated popup mutations do not rebuild or remeasure the bar inventory', async () => {
  const f = fixture();
  const changed = jest.fn();
  const release = f.backdrops.observe(changed);
  f.backdrops.read({ x: 10, y: 20 }, { width: 500, height: 400 });
  const reads = f.bounds.mock.calls.length;
  try {
    const popup = document.createElement('aside');
    document.body.append(popup);
    popup.style.width = '200px';
    f.bar.dataset.scNativeBackdrop = 'ready';
    await Promise.resolve();
    await Promise.resolve();
    expect(changed).not.toHaveBeenCalled();
    expect(f.bounds.mock.calls.length).toBe(reads);
    expect(f.backdrops.current).toBe(true);
  } finally {
    release();
  }
});

test('ancestor material CSS changes invalidate masks while a tint-only change does not', async () => {
  const f = fixture();
  const computed = window.getComputedStyle.bind(window);
  jest.spyOn(window, 'getComputedStyle').mockImplementation((element) => {
    const style = computed(element);
    if (element === f.bar) {
      style.setProperty(
        '--p-toast-blur',
        f.root.style.getPropertyValue('--p-toast-blur') || '12px',
      );
      style.setProperty('border-radius', f.root.style.getPropertyValue('--bar-radius') || '8px');
    }
    return style;
  });
  const changed = jest.fn();
  const release = f.backdrops.observe(changed);
  f.backdrops.read({ x: 10, y: 20 }, { width: 500, height: 400 });
  try {
    f.root.style.color = 'red';
    await Promise.resolve();
    await Promise.resolve();
    expect(changed).not.toHaveBeenCalled();
    expect(f.backdrops.current).toBe(true);
    f.root.style.setProperty('--p-toast-blur', '18px');
    await Promise.resolve();
    await Promise.resolve();
    expect(f.backdrops.current).toBe(false);
    expect(changed).toHaveBeenCalledTimes(1);
    expect(f.backdrops.read({ x: 10, y: 20 }, { width: 500, height: 400 })[0]?.sigma).toBe(18);
    f.root.style.setProperty('--bar-radius', '12px');
    f.root.dataset.scColorScheme = 'light';
    await Promise.resolve();
    await Promise.resolve();
    expect(f.backdrops.current).toBe(false);
    expect(f.backdrops.read({ x: 10, y: 20 }, { width: 500, height: 400 })[0]?.cornerRadius).toBe(
      12,
    );
  } finally {
    release();
  }
});

test('density changes withdraw readiness and keep regions in CSS pixels', () => {
  const f = fixture();
  const frames = new CanvasFrameFixture();
  const changed = jest.fn();
  const observer = new CanvasSurfaceGeometryObserver(
    f.viewport,
    frames,
    changed,
    null,
    f.backdrops,
  );
  try {
    observer.read();
    jest.replaceProperty(window, 'devicePixelRatio', 2);
    window.dispatchEvent(new Event('resize'));
    expect(f.backdrops.current).toBe(false);
    frames.advance(16);
    expect(changed.mock.calls[0]?.[0]).toMatchObject({
      physicalPixelsPerCssPixel: 2,
      backdrops: [{ x: 8, y: 350, width: 200, height: 42 }],
    });
  } finally {
    observer.dispose();
  }
});

test('unrepresentable or excessive material layouts remain opaque instead of publishing stale masks', async () => {
  const f = fixture();
  const release = f.backdrops.observe(() => {});
  try {
    f.bounds.mockReturnValue(rect(-2, 370, 200, 42));
    expect(f.backdrops.read({ x: 10, y: 20 }, { width: 500, height: 400 })).toEqual([]);
    expect(f.backdrops.current).toBe(false);
    for (let index = 0; index < 2; index++) {
      const bar = f.bar.cloneNode() as HTMLElement;
      f.root.append(bar);
    }
    await Promise.resolve();
    await Promise.resolve();
    expect(f.backdrops.read({ x: 10, y: 20 }, { width: 500, height: 400 })).toEqual([]);
    expect(f.backdrops.current).toBe(false);
  } finally {
    release();
  }
});

test('selection and comparison control changes keep unchanged acknowledged material current', async () => {
  const f = fixture();
  const changed = jest.fn();
  const release = f.backdrops.observe(changed);
  const button = document.createElement('button');
  const slider = document.createElement('input');
  slider.type = 'range';
  f.bar.append(button, slider);
  f.backdrops.read({ x: 10, y: 20 }, { width: 500, height: 400 });
  const resize = ResizeBoundary.instances.find((boundary) => boundary.elements.has(f.bar))!;
  const rebound = jest.spyOn(resize, 'disconnect');
  const inventory = jest.spyOn(f.root, 'querySelectorAll');
  try {
    for (const selection of ['output-a', 'output-b', 'compare-a', 'compare-b']) {
      button.className = selection;
      slider.classList.toggle('active');
      slider.style.setProperty('--split', '0.75');
      await Promise.resolve();
      await Promise.resolve();
      expect(f.backdrops.current).toBe(true);
      expect(f.invalidated).not.toHaveBeenCalled();
      expect(changed).not.toHaveBeenCalled();
      expect(rebound).not.toHaveBeenCalled();
      expect(inventory).not.toHaveBeenCalled();
    }
    f.bounds.mockReturnValue(rect(18, 370, 240, 42));
    button.className = 'wider-source';
    await Promise.resolve();
    await Promise.resolve();
    expect(f.backdrops.current).toBe(false);
    expect(f.invalidated).toHaveBeenCalledTimes(1);
    expect(changed).toHaveBeenCalledTimes(1);
  } finally {
    release();
  }
});

test('a scene-only bar mounted after empty startup reaches native geometry without reopening the canvas', async () => {
  const f = fixture();
  f.bar.remove();
  const frames = new CanvasFrameFixture();
  const changed = jest.fn();
  const observer = new CanvasSurfaceGeometryObserver(
    f.viewport,
    frames,
    changed,
    null,
    f.backdrops,
  );
  try {
    expect(observer.read().backdrops).toEqual([]);
    f.root.append(f.bar);
    await Promise.resolve();
    await Promise.resolve();
    frames.advance(16);
    expect(changed.mock.calls.at(-1)?.[0]).toMatchObject({
      backdrops: [{ width: 200, sigma: 12 }],
    });
    expect(f.backdrops.current).toBe(true);
  } finally {
    observer.dispose();
  }
});

test('a late bar mounted and styled in one mutation delivery enters the material inventory', async () => {
  const f = fixture();
  f.bar.remove();
  const frames = new CanvasFrameFixture();
  const changed = jest.fn();
  const observer = new CanvasSurfaceGeometryObserver(
    f.viewport,
    frames,
    changed,
    null,
    f.backdrops,
  );
  try {
    expect(observer.read().backdrops).toEqual([]);
    f.root.append(f.bar);
    f.bar.classList.add('sc-output-navigation');
    f.bar.style.borderRadius = '12px';
    await Promise.resolve();
    await Promise.resolve();
    frames.advance(16);
    expect(f.backdrops.diagnostics()).toMatchObject({
      bars: 1,
      current: true,
      measured: { regions: [{ width: 200, cornerRadius: 12 }] },
    });
    expect(changed.mock.calls.at(-1)?.[0]).toMatchObject({
      backdrops: [{ width: 200, cornerRadius: 12 }],
    });
  } finally {
    observer.dispose();
  }
});

test('a removed bar with a later styling notification retires from the material inventory', async () => {
  const f = fixture();
  const release = f.backdrops.observe(() => {});
  f.backdrops.read({ x: 10, y: 20 }, { width: 500, height: 400 });
  const resize = ResizeBoundary.instances.find((boundary) => boundary.elements.has(f.bar))!;
  try {
    f.bar.remove();
    f.bar.style.borderRadius = '12px';
    await Promise.resolve();
    await Promise.resolve();
    expect(f.backdrops.diagnostics().bars).toBe(0);
    expect(resize.elements.has(f.bar)).toBe(false);
    expect(f.backdrops.read({ x: 10, y: 20 }, { width: 500, height: 400 })).toEqual([]);
    expect(f.backdrops.current).toBe(true);
  } finally {
    release();
  }
});

test('a relocated and styled bar keeps its resize lifetime and observes its new material ancestry', async () => {
  const f = fixture();
  const wrapper = document.createElement('section');
  f.root.append(wrapper);
  const release = f.backdrops.observe(() => {});
  f.backdrops.read({ x: 10, y: 20 }, { width: 500, height: 400 });
  const resize = ResizeBoundary.instances.find((boundary) => boundary.elements.has(f.bar))!;
  const rebound = jest.spyOn(resize, 'disconnect');
  try {
    wrapper.append(f.bar);
    f.bar.style.borderRadius = '12px';
    await Promise.resolve();
    await Promise.resolve();
    expect(f.backdrops.diagnostics().bars).toBe(1);
    expect(rebound).not.toHaveBeenCalled();
    expect(f.backdrops.read({ x: 10, y: 20 }, { width: 500, height: 400 })[0]?.cornerRadius).toBe(
      12,
    );
    f.bounds.mockReturnValue(rect(18, 370, 240, 42));
    wrapper.style.padding = '2px';
    await Promise.resolve();
    await Promise.resolve();
    expect(f.backdrops.current).toBe(false);
    expect(f.backdrops.read({ x: 10, y: 20 }, { width: 500, height: 400 })[0]?.width).toBe(240);
  } finally {
    release();
  }
});

test('a staged hidden bar is remeasured after its actual canvas root becomes visible', async () => {
  const f = fixture();
  f.root.style.visibility = 'hidden';
  const frames = new CanvasFrameFixture();
  const changed = jest.fn();
  const observer = new CanvasSurfaceGeometryObserver(
    f.viewport,
    frames,
    changed,
    null,
    f.backdrops,
  );
  try {
    expect(observer.read().backdrops).toEqual([]);
    f.root.style.visibility = '';
    await Promise.resolve();
    await Promise.resolve();
    frames.advance(16);
    expect(changed.mock.calls.at(-1)?.[0]).toMatchObject({
      visible: true,
      backdrops: [{ width: 200, sigma: 12 }],
    });
    expect(f.backdrops.current).toBe(true);
  } finally {
    observer.dispose();
  }
});

test('material diagnostics expose only cached bounded facts and never perform a layout read', () => {
  const f = fixture();
  const observer = new CanvasSurfaceGeometryObserver(
    f.viewport,
    new CanvasFrameFixture(),
    () => {},
    null,
    f.backdrops,
  );
  try {
    observer.read();
    const reads = f.bounds.mock.calls.length;
    const diagnostics = f.backdrops.diagnostics();
    expect(diagnostics).toMatchObject({
      observing: true,
      dirty: false,
      complete: true,
      bars: 1,
      current: true,
      measured: { regions: [{ width: 200, sigma: 12 }] },
    });
    expect(Object.isFrozen(diagnostics.measured?.regions)).toBe(true);
    for (let step = 0; step < 10; step++) f.backdrops.diagnostics();
    expect(f.bounds.mock.calls.length).toBe(reads);
  } finally {
    observer.dispose();
  }
});
