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
/** Verify exact native builders, paint suspension, bounded capture, and reversible host hooks. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { captureMasonryLinkPaths } from '../../../frontend/comfyui/ui/surface/masonry/ComfyMasonryLinkAdapter.js';
import { NativeLinkPathRecording } from '../../../frontend/comfyui/ui/surface/masonry/NativeLinkPathRecording.js';

const originalContext = Object.getOwnPropertyDescriptor(globalThis, 'CanvasRenderingContext2D');
afterEach(() => {
  if (originalContext)
    Object.defineProperty(globalThis, 'CanvasRenderingContext2D', originalContext);
  else Reflect.deleteProperty(globalThis, 'CanvasRenderingContext2D');
  document.body.replaceChildren();
  jest.restoreAllMocks();
});

/** Model the external native paint boundary while using real recording and hook ownership. */
function fixture(count = 3) {
  class Context {
    canvas = document.createElement('canvas');
    globalAlpha = 0.75;
    lineCap = 'round';
    stroke = jest.fn();
    getTransform() {
      return { a: 2, b: 0, c: 0, d: 2, e: 20, f: 30 };
    }
  }
  Object.defineProperty(globalThis, 'CanvasRenderingContext2D', {
    configurable: true,
    value: Context,
  });
  const context = new Context();
  context.canvas.width = 200;
  const builder = jest.fn((path: unknown) => {
    if (!(path instanceof NativeLinkPathRecording)) throw new Error('Invalid recorder');
    path.moveTo(1, 2);
    path.bezierCurveTo(3, 4, 5, 6, 7, 8);
  });
  const drawPath = jest.fn(() => {
    context.stroke();
  });
  const renderer = {
    drawLinkPath: drawPath,
    buildSplinePath: builder,
    buildStraightPath: builder,
    buildLinearPath: builder,
  };
  const container = document.createElement('div');
  const nativeCanvas = document.createElement('canvas');
  nativeCanvas.width = 200;
  document.body.append(container, nativeCanvas);
  jest.spyOn(container, 'getBoundingClientRect').mockReturnValue(bounds(400, 300));
  jest.spyOn(nativeCanvas, 'getBoundingClientRect').mockReturnValue(bounds(200, 150));
  const draw = jest.fn(() => {
    const path: unknown = renderer.drawLinkPath;
    if (typeof path !== 'function') throw new Error('Missing native path draw');
    for (let index = 0; index < count; index++)
      Reflect.apply(path, renderer, [
        context,
        {},
        {
          id: String(index),
          startPoint: { x: 1, y: 2 },
          endPoint: { x: 7, y: 8 },
          startDirection: 'right',
          endDirection: 'left',
        },
        { style: { mode: ['spline', 'straight', 'linear'][index % 3] } },
        2,
        '#aabbcc',
      ]);
  });
  const canvas = { canvas: nativeCanvas, linkRenderer: { pathRenderer: renderer }, draw };
  return { container, canvas, context, renderer, drawPath, draw, builder };
}

/** Supply the browser rectangle contract in the DOM-only test environment. */
function bounds(width: number, height: number): DOMRect {
  return {
    x: 10,
    y: 20,
    left: 10,
    top: 20,
    right: 10 + width,
    bottom: 20 + height,
    width,
    height,
    toJSON: () => ({}),
  };
}

test('captures actual native trajectories and paint styles while preserving the original path calls', () => {
  const { container, canvas, context, renderer, drawPath, draw, builder } = fixture();
  const snapshot = captureMasonryLinkPaths(canvas, container);
  expect(snapshot?.strokes).toHaveLength(3);
  expect(snapshot?.strokes[0]).toEqual({
    link: '0',
    path: 'M1 2 C3 4 5 6 7 8',
    transform: 'matrix(2 0 0 2 20 30)',
    color: '#aabbcc',
    width: 2,
    opacity: 0.75,
    lineCap: 'round',
  });
  expect(context.stroke).not.toHaveBeenCalled();
  expect(drawPath).toHaveBeenCalledTimes(3);
  expect(builder).toHaveBeenCalledTimes(3);
  canvas.draw();
  expect(builder).toHaveBeenCalledTimes(3);
  expect(context.stroke).not.toHaveBeenCalled();
  snapshot?.restore();
  snapshot?.restore();
  expect(renderer.drawLinkPath).toBe(drawPath);
  expect(context.stroke).toHaveBeenCalledTimes(3);
  expect(draw).toHaveBeenCalledTimes(3);
});

test('restores native paint after capture exceeds its temporary vector budget', () => {
  const { container, canvas, context, renderer, drawPath } = fixture(513);
  expect(captureMasonryLinkPaths(canvas, container)).toBeNull();
  expect(renderer.drawLinkPath).toBe(drawPath);
  expect(context.stroke).toHaveBeenCalledTimes(513);
});

test('undoes Comfy background-buffer DPR when copying routes into screen coordinates', () => {
  const { container, canvas, context } = fixture();
  const ratio = Object.getOwnPropertyDescriptor(window, 'devicePixelRatio');
  Object.defineProperty(window, 'devicePixelRatio', { configurable: true, value: 0.9 });
  try {
    const snapshot = captureMasonryLinkPaths({ ...canvas, bgcanvas: context.canvas }, container);
    expect(snapshot?.strokes[0]?.transform).toBe(
      `matrix(${2 / 0.9} 0 0 ${2 / 0.9} ${20 / 0.9} ${30 / 0.9})`,
    );
    snapshot?.restore();
  } finally {
    if (ratio) Object.defineProperty(window, 'devicePixelRatio', ratio);
    else Reflect.deleteProperty(window, 'devicePixelRatio');
  }
});

test('restores the host after a failed native builder and rejects invalid path coordinates', () => {
  const { container, canvas, context, renderer, drawPath, builder } = fixture();
  builder.mockImplementation(() => {
    throw new Error('Unsupported route');
  });
  expect(() => captureMasonryLinkPaths(canvas, container)).toThrow('Unsupported route');
  expect(renderer.drawLinkPath).toBe(drawPath);
  expect(context.stroke).toHaveBeenCalledTimes(3);
  expect(() => new NativeLinkPathRecording().lineTo(Number.NaN, 1)).toThrow('invalid coordinates');
});

test('leaves unavailable native capabilities untouched', () => {
  const container = document.createElement('div');
  const draw = jest.fn();
  expect(captureMasonryLinkPaths({ draw }, container)).toBeNull();
  expect(draw).not.toHaveBeenCalled();
});

test('suppresses native markers during travel and preserves a subsequently composed host hook', () => {
  const { container, canvas, renderer, context } = fixture();
  const drawLink = jest.fn<(...arguments_: unknown[]) => unknown>();
  Object.defineProperty(renderer, 'drawLink', { configurable: true, value: drawLink });
  const snapshot = captureMasonryLinkPaths(canvas, container);
  const options = { animation: { time: 1 }, style: { showArrows: true, showCenterMarker: true } };
  const drawing: unknown = Reflect.get(renderer, 'drawLink');
  if (typeof drawing !== 'function') throw new Error('Native draw is missing');
  Reflect.apply(drawing, renderer, [context, {}, options]);
  expect(drawLink.mock.calls[0]?.[2]).toEqual({
    animation: undefined,
    style: { showArrows: false, showCenterMarker: false },
  });
  expect(options.style).toEqual({ showArrows: true, showCenterMarker: true });
  const previous: unknown = renderer.drawLinkPath;
  if (typeof previous !== 'function') throw new Error('Path draw is missing');
  const composed = (...arguments_: unknown[]): unknown =>
    Reflect.apply(previous, renderer, arguments_);
  Object.defineProperty(renderer, 'drawLinkPath', { configurable: true, value: composed });
  snapshot?.restore();
  expect(renderer.drawLinkPath).toBe(composed);
  expect(Reflect.get(renderer, 'drawLink')).toBe(drawLink);
  expect(context.stroke).toHaveBeenCalledTimes(3);
});

test('maps each canvas axis independently when CSS dimensions differ from the paint buffer', () => {
  const { container, canvas, context } = fixture();
  context.canvas.height = 300;
  const snapshot = captureMasonryLinkPaths(canvas, container);
  expect(snapshot?.strokes[0]?.transform).toBe('matrix(2 0 0 1 20 15)');
  snapshot?.restore();
});
