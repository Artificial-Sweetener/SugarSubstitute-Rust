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
import { afterEach, beforeEach, describe, expect, jest, test } from '@jest/globals';
import { CubePreviewRenderer } from '../../web/comfyui/ui/browser/CubePreviewRenderer.js';
import type {
  PreviewAdapter,
  PreviewBounds,
  PreviewHelpers,
  PreviewPayload,
  PreviewPlacementActions,
} from '../../web/comfyui/ui/browser/CubePreviewRenderer.js';
import type { Vec2 } from '../../web/comfyui/ui/types/common.js';

interface HarnessOverrides {
  placement?: Partial<PreviewPlacementActions>;
  adapter?: Partial<PreviewAdapter>;
}

interface RendererInternals {
  canvas: HTMLCanvasElement | null;
  payload: PreviewPayload | null;
  pointerHandlersBound: boolean;
  dragging: boolean;
  dragPointerId: number | null;
  dragOrigin: Vec2 | null;
}

interface ResizeObserverMock {
  callback: ResizeObserverCallback;
  observe: jest.Mock<(target: Element) => void>;
  disconnect: jest.Mock<() => void>;
}

function createContext() {
  const context = {
    save: jest.fn(),
    restore: jest.fn(),
    translate: jest.fn(),
    scale: jest.fn(),
    setTransform: jest.fn(),
    clearRect: jest.fn(),
  };
  return context as typeof context & CanvasRenderingContext2D;
}

function createHarness(overrides: HarnessOverrides = {}) {
  const ctx = createContext();
  const canvas = document.createElement('canvas');
  canvas.getContext = jest.fn(() => ctx) as unknown as typeof canvas.getContext;
  canvas.setPointerCapture = jest.fn();
  canvas.releasePointerCapture = jest.fn();
  const container = document.createElement('div');
  Object.defineProperty(container, 'clientWidth', { configurable: true, value: 216 });
  const status = document.createElement('div');
  const helpers = {
    computePayloadBounds: jest.fn<NonNullable<PreviewHelpers['computePayloadBounds']>>(() => ({
      minX: 0,
      minY: 0,
      maxX: 100,
      maxY: 50,
    })),
    readVector2: jest.fn<NonNullable<PreviewHelpers['readVector2']>>((value, x, y) =>
      Array.isArray(value) ? [Number(value[0]), Number(value[1])] : [x, y],
    ),
    coerceVec2: jest.fn<NonNullable<PreviewHelpers['coerceVec2']>>((value) =>
      Array.isArray(value) && value.length >= 2 ? [Number(value[0]), Number(value[1])] : null,
    ),
    resolvePreviewRect: jest.fn<NonNullable<PreviewHelpers['resolvePreviewRect']>>(
      (_entry, pos, size) => ({
        x: pos[0],
        y: pos[1],
        w: size[0],
        h: size[1],
      }),
    ),
    drawGhostRect: jest.fn<NonNullable<PreviewHelpers['drawGhostRect']>>(),
    getPlacementGroupLabel: jest.fn<NonNullable<PreviewHelpers['getPlacementGroupLabel']>>(
      () => 'Group label',
    ),
  };
  const placement: PreviewPlacementActions = {
    computeOriginFromEvent: jest.fn((): Vec2 => [12, 34]),
    getState: jest.fn(() => ({ active: true, cubeId: 'cube-a' })),
    setOrigin: jest.fn(),
    setDirty: jest.fn(),
    start: jest.fn(() => Promise.resolve()),
    stop: jest.fn(),
    isPointerOverCanvas: jest.fn(() => true),
    setCommitInProgress: jest.fn(),
    commit: jest.fn(() => Promise.resolve()),
    ...overrides.placement,
  };
  const adapter: PreviewAdapter = {
    getWindow: jest.fn(() => ({ devicePixelRatio: 2 })),
    ...overrides.adapter,
  };
  const renderer = new CubePreviewRenderer({ adapter, helpers, placement });
  return { adapter, canvas, container, ctx, helpers, placement, renderer, status };
}

function pointerEvent(overrides: Partial<PointerEvent> = {}): PointerEvent {
  const event = new MouseEvent('pointerdown', {
    bubbles: true,
    cancelable: true,
    button: overrides.button ?? 0,
    clientX: overrides.clientX ?? 20,
    clientY: overrides.clientY ?? 30,
  }) as PointerEvent;
  Object.defineProperty(event, 'pointerId', { value: overrides.pointerId ?? 7 });
  Object.defineProperty(event, 'preventDefault', { value: jest.fn() });
  Object.defineProperty(event, 'stopPropagation', { value: jest.fn() });
  return event;
}

describe('cube preview renderer characterization', () => {
  let resizeObservers: ResizeObserverMock[];

  beforeEach(() => {
    resizeObservers = [];
    globalThis.ResizeObserver = class ResizeObserverMockClass {
      callback: ResizeObserverCallback;
      observe: jest.Mock<(target: Element) => void>;
      disconnect: jest.Mock<() => void>;

      constructor(callback: ResizeObserverCallback) {
        this.callback = callback;
        this.observe = jest.fn();
        this.disconnect = jest.fn();
        resizeObservers.push(this);
      }
    } as unknown as typeof ResizeObserver;
  });

  afterEach(() => {
    Reflect.deleteProperty(globalThis, 'ResizeObserver');
  });

  test('attach and dispose own resize and pointer-handler lifecycles', () => {
    const { canvas, container, renderer, status } = createHarness();
    const removeEventListener = jest.spyOn(canvas, 'removeEventListener');

    renderer.attach({ canvas, container, status });
    const internals = renderer as unknown as RendererInternals;

    expect(resizeObservers).toHaveLength(1);
    expect(resizeObservers[0].observe).toHaveBeenCalledWith(container);
    expect(internals.pointerHandlersBound).toBe(true);

    renderer.dispose();

    expect(resizeObservers[0].disconnect).toHaveBeenCalledTimes(1);
    expect(removeEventListener).toHaveBeenCalledTimes(4);
    expect(internals.canvas).toBeNull();
    expect(internals.payload).toBeNull();
  });

  test.each([
    [{ loading: true }, 'Loading preview...'],
    [{ error: 'Preview failed' }, 'Preview failed'],
    [{}, 'Select a cube to see the layout preview.'],
  ])('missing preview payload renders the established status', (state, expected) => {
    const { canvas, container, renderer, status } = createHarness();
    renderer.attach({ canvas, container, status });

    renderer.update(state);

    expect(canvas.style.display).toBe('none');
    expect(status.textContent).toBe(expected);
    expect(status.style.display).toBe('block');
  });

  test('preview bounds combine node geometry with layout groups at the base origin', () => {
    const { ctx, helpers, renderer } = createHarness();
    helpers.computePayloadBounds.mockReturnValue(null);
    const payload = {
      nodes: [{ layout: { pos: [1, 2], size: [3, 4] } }],
      markers: [],
      layout: { origin: [100, 200], groups: [{ bounding: [10, 20, 30, 40] }] },
    };

    expect(renderer.computePreviewBounds(payload, ctx)).toEqual({
      minX: 110,
      minY: 220,
      maxX: 140,
      maxY: 260,
    });
  });

  test('render sizes the canvas for device pixels and delegates drawing', () => {
    const { canvas, container, ctx, renderer, status } = createHarness();
    renderer.attach({ canvas, container, status });
    const drawCubePreview = jest.spyOn(renderer, 'drawCubePreview');
    const payload = { nodes: [], markers: [], layout: { origin: [0, 0], groups: [] } };

    renderer.update({ payload, name: 'Demo', requestKey: 'demo' });

    expect(canvas.style.display).toBe('block');
    expect(canvas.style.height).toBe('120px');
    expect(canvas.width).toBe(400);
    expect(canvas.height).toBe(240);
    expect(ctx.setTransform).toHaveBeenCalledWith(2, 0, 0, 2, 0, 0);
    expect(ctx.clearRect).toHaveBeenCalledWith(0, 0, 200, 120);
    expect(drawCubePreview).toHaveBeenCalledWith(
      ctx,
      payload,
      { minX: 0, minY: 0, maxX: 100, maxY: 50 },
      { width: 200, height: 120 },
      'Demo',
    );
    expect(status.style.display).toBe('none');
  });

  test('render hides payloads without usable layout bounds', () => {
    const { canvas, container, helpers, renderer, status } = createHarness();
    helpers.computePayloadBounds.mockReturnValue(null);
    renderer.attach({ canvas, container, status });

    renderer.update({ payload: { nodes: [], markers: [] } });

    expect(canvas.style.display).toBe('none');
    expect(status.textContent).toBe('No layout preview available.');
  });

  test('drawing distinguishes groups, nodes, input markers, and output markers', () => {
    const { ctx, helpers, renderer } = createHarness();
    const payload = {
      nodes: [{ layout: { pos: [1, 2], size: [30, 40] } }],
      markers: [
        { kind: 'input', layout: { pos: [5, 6], size: [10, 11] } },
        { kind: 'output', layout: { pos: [15, 16], size: [12, 13] } },
      ],
      layout: { origin: [100, 200], groups: [{ bounding: [10, 20, 50, 60] }] },
    };

    renderer.drawCubePreview(
      ctx,
      payload,
      { minX: 0, minY: 0, maxX: 200, maxY: 300 },
      { width: 400, height: 400 },
      'Demo',
    );

    expect(helpers.drawGhostRect).toHaveBeenCalledTimes(4);
    expect(helpers.drawGhostRect.mock.calls[0][1]).toEqual({
      x: 110,
      y: 220,
      w: 50,
      h: 60,
    });
    expect(helpers.drawGhostRect.mock.calls[0][4]).toBe('Group label');
    expect(helpers.drawGhostRect.mock.calls[2][2].stroke).toBe('rgba(90, 200, 120, 0.7)');
    expect(helpers.drawGhostRect.mock.calls[3][2].stroke).toBe('rgba(90, 140, 240, 0.7)');
    expect(ctx.save).toHaveBeenCalledTimes(1);
    expect(ctx.restore).toHaveBeenCalledTimes(1);
  });

  test('primary pointer drag starts placement and continuously forwards its origin', async () => {
    const { canvas, container, placement, renderer, status } = createHarness();
    renderer.attach({ canvas, container, status });
    renderer.setContext({ selectedId: 'cube-a', selected: 'Demo', busy: false });
    const event = pointerEvent();

    renderer.handlePointerDown(event);
    await Promise.resolve();

    expect(event.preventDefault).toHaveBeenCalledTimes(1);
    expect(event.stopPropagation).toHaveBeenCalledTimes(1);
    expect(canvas.setPointerCapture).toHaveBeenCalledWith(7);
    expect(canvas.classList.contains('is-dragging')).toBe(true);
    expect(placement.start).toHaveBeenCalledWith('cube-a', {
      closeBrowser: false,
      defaultAlias: 'Demo',
    });
    expect(placement.setOrigin).toHaveBeenCalledWith([12, 34]);
    expect(placement.setDirty).toHaveBeenCalled();
  });

  test('pointer release over the graph commits and clears commit state asynchronously', async () => {
    const { canvas, placement, renderer } = createHarness();
    const internals = renderer as unknown as RendererInternals;
    internals.canvas = canvas;
    internals.dragging = true;
    internals.dragPointerId = 7;
    internals.dragOrigin = [12, 34];

    renderer.handlePointerUp(pointerEvent());
    await Promise.resolve();
    await Promise.resolve();

    expect(canvas.releasePointerCapture).toHaveBeenCalledWith(7);
    expect(placement.setCommitInProgress).toHaveBeenNthCalledWith(1, true);
    expect(placement.commit).toHaveBeenCalledTimes(1);
    expect(placement.setCommitInProgress).toHaveBeenLastCalledWith(false);
    expect(placement.stop).not.toHaveBeenCalled();
  });

  test('cancelled or off-canvas pointer drags stop placement without committing', () => {
    const { canvas, placement, renderer } = createHarness({
      placement: { isPointerOverCanvas: jest.fn(() => false) },
    });
    const internals = renderer as unknown as RendererInternals;
    internals.canvas = canvas;
    internals.dragging = true;
    internals.dragPointerId = 7;

    renderer.handlePointerCancel(pointerEvent());

    expect(placement.commit).not.toHaveBeenCalled();
    expect(placement.stop).toHaveBeenCalledWith('Placement cancelled.');
  });

  test('busy, unselected, and non-primary pointer input cannot begin placement', () => {
    const { canvas, container, placement, renderer, status } = createHarness();
    renderer.attach({ canvas, container, status });

    renderer.setContext({ selectedId: 'cube-a', selected: 'Demo', busy: true });
    renderer.handlePointerDown(pointerEvent());
    renderer.setContext({ selectedId: null, busy: false });
    renderer.handlePointerDown(pointerEvent());
    renderer.setContext({ selectedId: 'cube-a' });
    renderer.handlePointerDown(pointerEvent({ button: 2 }));

    expect(placement.start).not.toHaveBeenCalled();
    expect(canvas.setPointerCapture).not.toHaveBeenCalled();
  });
});
