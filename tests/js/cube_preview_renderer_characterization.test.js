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

function createContext() {
  return {
    save: jest.fn(),
    restore: jest.fn(),
    translate: jest.fn(),
    scale: jest.fn(),
    setTransform: jest.fn(),
    clearRect: jest.fn(),
  };
}

function createHarness(overrides = {}) {
  const ctx = createContext();
  const canvas = document.createElement('canvas');
  canvas.getContext = jest.fn(() => ctx);
  canvas.setPointerCapture = jest.fn();
  canvas.releasePointerCapture = jest.fn();
  const container = document.createElement('div');
  Object.defineProperty(container, 'clientWidth', { configurable: true, value: 216 });
  const status = document.createElement('div');
  const helpers = {
    computePayloadBounds: jest.fn(() => ({ minX: 0, minY: 0, maxX: 100, maxY: 50 })),
    readVector2: jest.fn((value, x, y) => (Array.isArray(value) ? value : [x, y])),
    coerceVec2: jest.fn((value) => (Array.isArray(value) && value.length >= 2 ? value : null)),
    resolvePreviewRect: jest.fn((_entry, pos, size) => ({
      x: pos[0],
      y: pos[1],
      w: size[0],
      h: size[1],
    })),
    drawGhostRect: jest.fn(),
    getPlacementGroupLabel: jest.fn(() => 'Group label'),
  };
  const placement = {
    computeOriginFromEvent: jest.fn(() => [12, 34]),
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
  const adapter = {
    getWindow: jest.fn(() => ({ devicePixelRatio: 2 })),
    ...overrides.adapter,
  };
  const renderer = new CubePreviewRenderer({ adapter, helpers, placement });
  return { adapter, canvas, container, ctx, helpers, placement, renderer, status };
}

function pointerEvent(overrides = {}) {
  return {
    button: 0,
    pointerId: 7,
    clientX: 20,
    clientY: 30,
    preventDefault: jest.fn(),
    stopPropagation: jest.fn(),
    ...overrides,
  };
}

describe('cube preview renderer characterization', () => {
  let resizeObservers;

  beforeEach(() => {
    resizeObservers = [];
    globalThis.ResizeObserver = class ResizeObserver {
      constructor(callback) {
        this.callback = callback;
        this.observe = jest.fn();
        this.disconnect = jest.fn();
        resizeObservers.push(this);
      }
    };
  });

  afterEach(() => {
    delete globalThis.ResizeObserver;
  });

  test('attach and dispose own resize and pointer-handler lifecycles', () => {
    const { canvas, container, renderer, status } = createHarness();
    const removeEventListener = jest.spyOn(canvas, 'removeEventListener');

    renderer.attach({ canvas, container, status });

    expect(resizeObservers).toHaveLength(1);
    expect(resizeObservers[0].observe).toHaveBeenCalledWith(container);
    expect(renderer.pointerHandlersBound).toBe(true);

    renderer.dispose();

    expect(resizeObservers[0].disconnect).toHaveBeenCalledTimes(1);
    expect(removeEventListener).toHaveBeenCalledTimes(4);
    expect(renderer.canvas).toBeNull();
    expect(renderer.payload).toBeNull();
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
    renderer.canvas = canvas;
    renderer.dragging = true;
    renderer.dragPointerId = 7;
    renderer.dragOrigin = [12, 34];

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
    renderer.canvas = canvas;
    renderer.dragging = true;
    renderer.dragPointerId = 7;

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
