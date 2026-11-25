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
/** Pin the Cube chain's one order, one placement and its wiring rules. */
import { describe, expect, test } from '@jest/globals';
import {
  cubeChainOrder,
  type ChainCube,
} from '../../../frontend/comfyui/ui/cube/chain/CubeChainOrder.js';
import {
  planChain,
  type PlacedFrame,
} from '../../../frontend/comfyui/ui/cube/chain/CubeChainPlacement.js';
import {
  planRewire,
  type RewireCube,
  type RewireLink,
} from '../../../frontend/comfyui/ui/cube/chain/CubeChainRewire.js';

describe('the stack order', () => {
  const cube = (id: string, x: number, y = 0): ChainCube => ({
    instanceId: id,
    nodeId: `node-${id}`,
    position: [x, y],
  });

  test('reads unconnected Cubes left to right, then top to bottom', () => {
    expect(cubeChainOrder([cube('c', 900), cube('a', 0), cube('b', 0, 500)], [])).toEqual([
      'a',
      'b',
      'c',
    ]);
  });

  test('puts a Cube after the Cube wired into it, wherever the two sit', () => {
    const edges = [{ source: 'node-right', target: 'node-left' }];
    expect(cubeChainOrder([cube('left', 0), cube('right', 900)], edges)).toEqual(['right', 'left']);
  });

  test('follows a wire through plain nodes between two Cubes', () => {
    const edges = [
      { source: 'node-right', target: 'upscale' },
      { source: 'upscale', target: 'node-left' },
    ];
    expect(cubeChainOrder([cube('left', 0), cube('right', 900)], edges)).toEqual(['right', 'left']);
  });

  test('counts a proximity link as a dependency like a wire', () => {
    const edges = [{ source: 'node-b', target: 'node-a' }];
    expect(cubeChainOrder([cube('a', 0), cube('b', 0, 400)], edges)).toEqual(['b', 'a']);
  });

  test('breaks a cycle at the leftmost Cube instead of losing Cubes', () => {
    const edges = [
      { source: 'node-a', target: 'node-b' },
      { source: 'node-b', target: 'node-a' },
    ];
    expect(cubeChainOrder([cube('b', 500), cube('a', 0)], edges)).toEqual(['a', 'b']);
  });
});

describe('the one placement', () => {
  const frame = (id: string, x: number, width: number, y = 0): PlacedFrame => ({
    id,
    position: [x, y],
    size: [width, 600],
  });
  const added = (id: string, width: number) => ({
    id,
    existing: false as const,
    size: [width, 600] as const,
  });
  const kept = (id: string) => ({ id, existing: true as const });

  test('two Cubes added in masonry land exactly where the add button puts them', () => {
    const fromDialog = planChain([], [added('t2i', 836), added('upscale', 1120)]);
    const first = planChain([], [added('t2i', 836)]);
    const fromButton = planChain(
      [frame('t2i', first.get('t2i')![0], 836)],
      [kept('t2i'), added('upscale', 1120)],
    );
    expect(fromDialog.get('t2i')).toEqual([0, 0]);
    expect(fromDialog.get('upscale')).toEqual([860, 0]);
    expect(fromButton.get('upscale')).toEqual(fromDialog.get('upscale'));
  });

  test('a Cube inserted mid-chain takes the space after its neighbour and moves the rest along', () => {
    const positions = planChain(
      [frame('source', 20, 220, 40), frame('downstream', 300, 360, 80)],
      [kept('source'), added('inserted', 340), kept('downstream')],
    );
    expect(positions.get('inserted')).toEqual([300, 40]);
    expect(positions.get('downstream')).toEqual([700, 80]);
  });

  test('reordered Cubes swap places and keep the chain spacing', () => {
    const positions = planChain([frame('a', 0, 400), frame('b', 500, 200)], [kept('b'), kept('a')]);
    expect(positions.get('b')).toEqual([0, 0]);
    expect(positions.get('a')).toEqual([300, 0]);
  });

  test('a removed Cube gives its place up and the chain closes behind it', () => {
    const positions = planChain(
      [frame('a', 0, 400), frame('b', 424, 400), frame('c', 848, 400)],
      [kept('a'), kept('c')],
    );
    expect(positions.get('c')).toEqual([424, 0]);
  });

  test('an unchanged chain keeps hand-placed positions', () => {
    const positions = planChain(
      [frame('a', 0, 400), frame('b', 1000, 400, 50)],
      [kept('a'), kept('b')],
    );
    expect(positions.get('b')).toEqual([1000, 50]);
  });

  test('a chain built from nothing starts where it is asked to', () => {
    const positions = planChain([], [added('a', 300), added('b', 300)], [100, 200]);
    expect(positions.get('a')).toEqual([100, 200]);
    expect(positions.get('b')).toEqual([424, 200]);
  });
});

describe('rewiring a hardwired stack', () => {
  const port = (slot: number, name: string, type = 'IMAGE') => ({ slot, name, type });
  const cubes: RewireCube[] = ['a', 'b', 'c'].map((id) => ({
    instanceId: id,
    title: id.toUpperCase(),
    inputs: [port(0, 'image')],
    outputs: [port(0, 'image')],
  }));
  const link = (source: string, target: string): RewireLink => ({
    sourceInstance: source,
    sourceSlot: 0,
    targetInstance: target,
    targetSlot: 0,
  });
  const accepts = (output: unknown, input: unknown) => output === input;

  test('a floating stack moves without touching any wire', () => {
    expect(planRewire(cubes, [], ['a', 'b', 'c'], ['a', 'c', 'b'], accepts)).toEqual({
      ok: true,
      remove: [],
      add: [],
    });
  });

  test('a hardwired Cube moved up the chain is wired to its new neighbours', () => {
    const plan = planRewire(
      cubes,
      [link('a', 'b'), link('b', 'c')],
      ['a', 'b', 'c'],
      ['a', 'c', 'b'],
      accepts,
    );
    expect(plan.ok).toBe(true);
    if (!plan.ok) return;
    expect(plan.remove).toEqual(expect.arrayContaining([link('a', 'b'), link('b', 'c')]));
    expect(plan.add).toEqual(expect.arrayContaining([link('a', 'c'), link('c', 'b')]));
  });

  test('a hardwired Cube cannot become first: nothing would feed its input', () => {
    const plan = planRewire(cubes, [link('a', 'b')], ['a', 'b', 'c'], ['b', 'a', 'c'], accepts);
    expect(plan).toEqual({
      ok: false,
      reason: 'B is wired to the Cube before it, so it cannot be first.',
    });
  });

  test('a move is refused when the new neighbour has no output that fits the wired input', () => {
    const mask: RewireCube = {
      instanceId: 'm',
      title: 'Mask',
      inputs: [],
      outputs: [port(0, 'mask', 'MASK')],
    };
    const plan = planRewire(
      [...cubes, mask],
      [link('a', 'b')],
      ['m', 'a', 'b'],
      ['a', 'm', 'b'],
      accepts,
    );
    expect(plan).toEqual({ ok: false, reason: "Mask has no output that fits B's wired input." });
  });

  test('a wire between Cubes that are not neighbours must still run forwards', () => {
    const plan = planRewire(cubes, [link('a', 'c')], ['a', 'b', 'c'], ['c', 'a', 'b'], accepts);
    expect(plan).toEqual({ ok: false, reason: 'C is wired to A, which would then run after it.' });
  });
});
