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
/** Verify transient Cube-port motion without involving either Comfy renderer. */

import { jest } from '@jest/globals';

import { CubePortPresentationController } from '../../frontend/comfyui/ui/cube/connection/CubePortPresentationController.js';
import type { ProximityMatch } from '../../frontend/comfyui/ui/overlays/proximity/ProximityModel.js';
import type { ComfyNode } from '../../frontend/comfyui/ui/types/graph.js';

describe('CubePortPresentationController', () => {
  test('resolves current animated graph geometry independently from canonical defaults', () => {
    let now = 0;
    const controller = new CubePortPresentationController({
      now: () => now,
      requestFrame: () => null,
      durationMs: 100,
    });
    const output = cube('output', 200);
    const input = cube('input', 300);
    controller.register(output, 'output', [
      { index: 0, defaultY: 100, minY: 50, maxY: 500, labelY: 100 },
    ]);
    controller.register(input, 'input', [
      { index: 0, defaultY: 100, minY: 50, maxY: 500, labelY: 100 },
    ]);
    controller.updateMatches([
      match({
        outputNode: output,
        outputCube: 'output-definition',
        outputSlot: 0,
        inputNode: input,
        inputCube: 'input-definition',
        inputSlot: 0,
      }),
    ]);

    now = 100;

    expect(controller.resolveGraphPosition(output, 'output', 0, [600, 300])).toEqual([600, 350]);
    expect(controller.resolveGraphPosition(input, 'input', 0, [650, 400])).toEqual([650, 350]);
    expect(controller.resolveDefaultGraphPosition(output, 'output', 0, [600, 300])).toEqual([
      600, 300,
    ]);
  });

  test('keeps disjoint Cube ports on their nearest boundary edges', () => {
    let now = 0;
    const controller = new CubePortPresentationController({
      now: () => now,
      requestFrame: () => null,
      durationMs: 1,
    });
    const output = cube('output', 100);
    const input = cube('input', 405);
    controller.register(output, 'output', [
      { index: 0, defaultY: 80, minY: 40, maxY: 300, labelY: 80 },
    ]);
    controller.register(input, 'input', [
      { index: 0, defaultY: 40, minY: 40, maxY: 300, labelY: 40 },
    ]);

    controller.updateMatches([
      match({
        outputNode: output,
        outputCube: 'output-definition',
        outputSlot: 0,
        inputNode: input,
        inputCube: 'input-definition',
        inputSlot: 0,
      }),
    ]);
    now = 1;

    expect(controller.resolveGraphPosition(output, 'output', 0, [500, 180])).toEqual([500, 400]);
    expect(controller.resolveGraphPosition(input, 'input', 0, [530, 445])).toEqual([530, 445]);
  });

  test('swaps visual output order without lifting a socket above its own label', () => {
    let now = 0;
    const invalidate = jest.fn();
    const controller = new CubePortPresentationController({
      now: () => now,
      requestFrame: () => null,
      invalidate,
      durationMs: 180,
    });
    const outputCube = cube('output-cube', 100);
    const inputCube = cube('input-cube', 120);
    controller.register(outputCube, 'output', [
      { index: 0, defaultY: 60, minY: 40, maxY: 280, labelY: 60 },
      { index: 1, defaultY: 180, minY: 40, maxY: 280, labelY: 180 },
    ]);
    controller.register(inputCube, 'input', [
      { index: 0, defaultY: 60, minY: 40, maxY: 280, labelY: 60 },
    ]);

    controller.updateMatches([
      match({
        outputNode: outputCube,
        outputCube: 'definition-output',
        outputSlot: 1,
        outputPos: [500, 280],
        inputNode: inputCube,
        inputCube: 'definition-input',
        inputSlot: 0,
        inputPos: [530, 180],
      }),
    ]);
    now = 180;

    expect(controller.resolveLocalY(outputCube, 'output', 1)).toBe(180);
    expect(controller.resolveLocalY(outputCube, 'output', 0)).toBe(198);
    expect(controller.resolveLocalY(inputCube, 'input', 0)).toBe(160);
    expect(controller.resolveDefaultGraphPosition(outputCube, 'output', 1, [500, 999])).toEqual([
      500, 280,
    ]);
    expect(invalidate).toHaveBeenCalled();
  });

  test('animates changed lanes and restores canonical defaults after separation', () => {
    let now = 0;
    const controller = new CubePortPresentationController({
      now: () => now,
      requestFrame: () => null,
      invalidate: () => undefined,
      durationMs: 200,
    });
    const outputCube = cube('output-cube', 100);
    const inputCube = cube('input-cube', 100);
    controller.register(outputCube, 'output', [
      { index: 0, defaultY: 50, minY: 30, maxY: 250, labelY: 50 },
      { index: 1, defaultY: 170, minY: 30, maxY: 250, labelY: 170 },
    ]);
    controller.register(inputCube, 'input', [
      { index: 0, defaultY: 50, minY: 30, maxY: 250, labelY: 50 },
    ]);
    controller.updateMatches([
      match({
        outputNode: outputCube,
        outputCube: 'output',
        outputSlot: 1,
        outputPos: [500, 270],
        inputNode: inputCube,
        inputCube: 'input',
        inputSlot: 0,
        inputPos: [530, 150],
      }),
    ]);

    now = 100;
    expect(controller.resolveLocalY(outputCube, 'output', 1)).toBe(170);
    now = 200;
    expect(controller.resolveLocalY(outputCube, 'output', 1)).toBe(170);

    controller.updateMatches([]);
    now = 300;
    expect(controller.resolveLocalY(outputCube, 'output', 1)).toBe(170);
    now = 400;
    expect(controller.resolveLocalY(outputCube, 'output', 0)).toBe(50);
    expect(controller.resolveLocalY(outputCube, 'output', 1)).toBe(170);
  });

  test('magnetizes a Cube output to an ordinary node input without persisting transient state', () => {
    let now = 0;
    const controller = new CubePortPresentationController({
      now: () => now,
      requestFrame: () => null,
      invalidate: () => undefined,
      durationMs: 100,
    });
    const cubeNode = cube('cube', 100);
    const ordinaryNode = { id: 'ordinary', pos: [600, 100] } as ComfyNode;
    controller.register(cubeNode, 'output', [
      { index: 0, defaultY: 80, minY: 40, maxY: 220, labelY: 80 },
    ]);
    controller.updateMatches([
      match({
        outputNode: cubeNode,
        outputCube: 'definition',
        outputSlot: 0,
        outputPos: [500, 180],
        inputNode: ordinaryNode,
        inputCube: null,
        inputSlot: 0,
        inputPos: [540, 240],
      }),
    ]);

    now = 100;

    expect(controller.resolveLocalY(cubeNode, 'output', 0)).toBe(140);
    expect(controller.serialize()).toEqual({});
  });

  test('magnetizes a Cube input to an ordinary node output', () => {
    let now = 0;
    const controller = new CubePortPresentationController({
      now: () => now,
      requestFrame: () => null,
      invalidate: () => undefined,
      durationMs: 100,
    });
    const ordinaryNode = { id: 'ordinary', pos: [100, 100] } as ComfyNode;
    const cubeNode = cube('cube', 100);
    controller.register(cubeNode, 'input', [
      { index: 0, defaultY: 160, minY: 40, maxY: 220, labelY: 160 },
    ]);
    controller.updateMatches([
      match({
        outputNode: ordinaryNode,
        outputCube: null,
        outputSlot: 0,
        outputPos: [500, 180],
        inputNode: cubeNode,
        inputCube: 'definition',
        inputSlot: 0,
        inputPos: [540, 260],
      }),
    ]);

    now = 100;

    expect(controller.resolveLocalY(cubeNode, 'input', 0)).toBe(80);
  });

  test('lifts a resized previewless Cube output above its new midpoint anchor', () => {
    let now = 0;
    const controller = new CubePortPresentationController({
      now: () => now,
      requestFrame: () => null,
      invalidate: () => undefined,
      durationMs: 100,
    });
    const cubeNode = cube('cube', 100);
    const ordinaryNode = { id: 'ordinary', pos: [600, 100] } as ComfyNode;
    controller.register(cubeNode, 'output', [
      { index: 0, defaultY: 300, minY: 40, maxY: 520, labelY: 40 },
    ]);
    controller.updateMatches([
      match({
        outputNode: cubeNode,
        outputCube: 'definition',
        outputSlot: 0,
        outputPos: [500, 400],
        inputNode: ordinaryNode,
        inputCube: null,
        inputSlot: 0,
        inputPos: [540, 240],
      }),
    ]);

    now = 100;

    expect(controller.resolveLocalY(cubeNode, 'output', 0)).toBe(140);
  });

  test('keeps matching geometry attached while a magnetized Cube is resized', () => {
    let now = 0;
    const controller = new CubePortPresentationController({
      now: () => now,
      requestFrame: () => null,
      invalidate: () => undefined,
      durationMs: 100,
    });
    const cubeNode = cube('cube', 100);
    const ordinaryNode = { id: 'ordinary', pos: [600, 100] } as ComfyNode;
    controller.register(cubeNode, 'output', [
      { index: 0, defaultY: 80, minY: 40, maxY: 220, labelY: 80 },
    ]);
    controller.updateMatches([
      match({
        outputNode: cubeNode,
        outputCube: 'definition',
        outputSlot: 0,
        outputPos: [500, 180],
        inputNode: ordinaryNode,
        inputCube: null,
        inputSlot: 0,
        inputPos: [540, 240],
      }),
    ]);
    now = 100;

    controller.register(cubeNode, 'output', [
      { index: 0, defaultY: 260, minY: 40, maxY: 400, labelY: 40 },
    ]);

    expect(controller.resolveDefaultGraphPosition(cubeNode, 'output', 0, [500, 0])).toEqual([
      500, 360,
    ]);
    expect(controller.resolveMatchingGraphPosition(cubeNode, 'output', 0, [500, 0])).toEqual([
      500, 240,
    ]);
  });

  test('keeps every temporarily reordered port inside a collision-free travel range', () => {
    let now = 0;
    const controller = new CubePortPresentationController({
      now: () => now,
      requestFrame: () => null,
      invalidate: () => undefined,
      durationMs: 1,
    });
    const outputCube = cube('output-cube', 0);
    const inputCube = cube('input-cube', 0);
    controller.register(
      outputCube,
      'output',
      [40, 50, 60, 70].map((defaultY, index) => ({
        index,
        defaultY,
        minY: 40,
        maxY: 70,
        labelY: defaultY,
      })),
    );
    controller.register(
      inputCube,
      'input',
      [70, 60, 50, 40].map((defaultY, index) => ({
        index,
        defaultY,
        minY: 40,
        maxY: 70,
        labelY: defaultY,
      })),
    );
    controller.updateMatches(
      [0, 1, 2, 3].map((index) =>
        match({
          outputNode: outputCube,
          outputCube: 'output',
          outputSlot: index,
          outputPos: [100, 40 + index * 10],
          inputNode: inputCube,
          inputCube: 'input',
          inputSlot: index,
          inputPos: [140, 70 - index * 10],
        }),
      ),
    );
    now = 1;

    const positions = [0, 1, 2, 3]
      .map((index) => controller.resolveLocalY(outputCube, 'output', index))
      .filter((position): position is number => position !== null)
      .sort((left, right) => left - right);

    expect(positions[0]).toBeGreaterThanOrEqual(40);
    expect(positions.at(-1)).toBeLessThanOrEqual(70);
    expect(positions.slice(1).every((position, index) => position > positions[index]!)).toBe(true);
  });

  test('publishes an exact final frame and stops scheduling after animation settles', () => {
    let now = 0;
    const frames: FrameRequestCallback[] = [];
    const listener = jest.fn();
    const controller = new CubePortPresentationController({
      now: () => now,
      requestFrame: (callback) => {
        frames.push(callback);
        return 1;
      },
      invalidate: () => undefined,
      durationMs: 180,
    });
    const outputCube = cube('output-cube', 100);
    const inputCube = cube('input-cube', 100);
    controller.register(outputCube, 'output', [
      { index: 0, defaultY: 60, minY: 40, maxY: 200, labelY: 60 },
    ]);
    controller.register(inputCube, 'input', [
      { index: 0, defaultY: 140, minY: 40, maxY: 200, labelY: 140 },
    ]);
    controller.subscribe(outputCube, listener);
    controller.updateMatches([
      match({
        outputNode: outputCube,
        outputCube: 'output',
        outputSlot: 0,
        outputPos: [500, 160],
        inputNode: inputCube,
        inputCube: 'input',
        inputSlot: 0,
        inputPos: [530, 240],
      }),
    ]);
    expect(controller.isAnimating(outputCube)).toBe(true);

    now = 180;
    frames[0]?.(now);

    expect(controller.resolveLocalY(outputCube, 'output', 0)).toBe(100);
    expect(controller.isAnimating(outputCube)).toBe(false);
    expect(listener).toHaveBeenCalledTimes(2);
  });

  test('animates from fallback anchors to preview geometry measured after mount', () => {
    let now = 0;
    const frames: FrameRequestCallback[] = [];
    const controller = new CubePortPresentationController({
      now: () => now,
      requestFrame: (callback) => {
        frames.push(callback);
        return frames.length;
      },
      invalidate: () => undefined,
      durationMs: 180,
    });
    const node = cube('cube', 0);
    controller.register(node, 'output', [
      { index: 0, defaultY: 120, minY: 40, maxY: 220, labelY: 120 },
    ]);

    controller.register(node, 'output', [
      { index: 0, defaultY: 50, minY: 40, maxY: 220, labelY: 50 },
    ]);

    expect(frames).toHaveLength(1);
    now = 180;
    frames[0]?.(now);
    expect(controller.resolveLocalY(node, 'output', 0)).toBe(50);
  });
});

/** Build the position surface used by transient local-to-graph conversion. */
function cube(id: string, y: number): ComfyNode {
  return { id, pos: [100, y], size: [400, 300] };
}

/** Build one complete routing match while allowing geometry-focused overrides. */
function match(overrides: Partial<ProximityMatch>): ProximityMatch {
  return {
    outputId: 'output-cube',
    outputSlot: 0,
    outputPos: [500, 160],
    inputId: 'input-cube',
    inputSlot: 0,
    inputName: 'image',
    inputPos: [530, 160],
    originId: 'producer',
    originSlot: 0,
    promptTargets: [{ nodeId: 'consumer', inputSlot: 0, inputName: 'image' }],
    distance: 30,
    ...overrides,
  };
}
