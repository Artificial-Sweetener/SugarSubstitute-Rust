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
/** A Cube loaded from an image written before 2026-11-25 shows its ports again. */
import { expect, test } from '@jest/globals';
import { repairCubeBoundaryLinks } from '../../../frontend/comfyui/ui/cube/CubeSerializedBoundaryRepair.js';

/** A Cube definition as those images stored it: boundaries named and typed, nothing else. */
function storedWorkflow() {
  return {
    nodes: [],
    links: [],
    definitions: {
      subgraphs: [
        {
          id: 'cube-upscale',
          name: 'Cube: Anima/Latent Upscale',
          extra: { sugarcubes_kind: 'cube' },
          inputNode: { id: -10 },
          outputNode: { id: -20 },
          inputs: [
            { name: 'input.image', type: 'IMAGE' },
            { name: 'input.mask', type: 'MASK' },
          ],
          outputs: [{ name: 'output.image', type: 'IMAGE' }],
          links: [
            {
              id: 3924,
              origin_id: -10,
              origin_slot: 0,
              target_id: 'encode',
              target_slot: 0,
              type: 'IMAGE',
            },
            {
              id: 3925,
              origin_id: -10,
              origin_slot: 1,
              target_id: 'mask',
              target_slot: 6,
              type: 'MASK',
            },
            {
              id: 3926,
              origin_id: 'decode',
              origin_slot: 0,
              target_id: -20,
              target_slot: 0,
              type: 'IMAGE',
            },
            {
              id: 3000,
              origin_id: 'a',
              origin_slot: 0,
              target_id: 'b',
              target_slot: 0,
              type: 'LATENT',
            },
          ],
        },
        {
          id: 'plain-subgraph',
          name: 'KSampler',
          extra: {},
          inputNode: { id: -10 },
          outputNode: { id: -20 },
          inputs: [{ name: 'model', type: 'MODEL' }],
          outputs: [],
          links: [
            {
              id: 1,
              origin_id: -10,
              origin_slot: 0,
              target_id: 'k',
              target_slot: 0,
              type: 'MODEL',
            },
          ],
        },
      ],
    },
  };
}

test('every Cube boundary gets back the links its definition draws to it', () => {
  const workflow = storedWorkflow();
  expect(repairCubeBoundaryLinks(workflow)).toBe(3);
  const [cube, plain] = workflow.definitions.subgraphs;
  expect(cube?.inputs.map((input) => (input as { linkIds?: number[] }).linkIds)).toEqual([
    [3924],
    [3925],
  ]);
  expect(cube?.outputs.map((output) => (output as { linkIds?: number[] }).linkIds)).toEqual([
    [3926],
  ]);
  // A plain subgraph is Comfy's own and is left alone.
  expect((plain?.inputs[0] as { linkIds?: number[] }).linkIds).toBeUndefined();
});

test('a boundary that lists its links is left exactly as it is', () => {
  const workflow = storedWorkflow();
  const output = workflow.definitions.subgraphs[0]!.outputs[0] as { linkIds?: number[] };
  output.linkIds = [];
  expect(repairCubeBoundaryLinks(workflow)).toBe(2);
  expect(output.linkIds).toEqual([]);
  expect(repairCubeBoundaryLinks(workflow)).toBe(0);
});
