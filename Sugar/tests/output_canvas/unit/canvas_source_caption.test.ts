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
/** Prove graph suppression outlives card mounts without retaining native interaction changes. */
import { expect, test } from '@jest/globals';
import {
  cubeOutputCaption,
  presentCanvasSources,
} from '../../../frontend/comfyui/ui/output_canvas/application/CanvasSourceCaption.js';
import { decodeComfyWorkflowSources } from '../../../frontend/comfyui/ui/execution/infrastructure/ComfyWorkflowSourceDecoder.js';

/** A saved workflow of Cubes, each with the outputs named. */
function workflow(
  cubes: readonly { id: number; title: string; model: string; outputs: string[] }[],
) {
  return {
    nodes: cubes.map((cube) => ({
      id: cube.id,
      type: `definition-${String(cube.id)}`,
      title: cube.title,
      properties: { sugarcubes_kind: 'cube', sugarcubes_cube: { target_model: cube.model } },
    })),
    definitions: {
      subgraphs: cubes.map((cube) => ({
        id: `definition-${String(cube.id)}`,
        name: cube.title,
        nodes: [],
        outputs: cube.outputs.map((name) => ({ name })),
      })),
    },
  };
}

test('a Cube output is captioned by the Cube’s own name, without its model folder or its output', () => {
  expect(
    cubeOutputCaption({
      title: 'Anima/Text to Image',
      name: 'Text to Image',
      outputName: 'image',
      outputCount: 1,
    }),
  ).toEqual({ label: 'Text to Image', breadcrumb: ['Anima/Text to Image', 'image'] });
  // Only a Cube with several outputs names the output, so its tabs differ.
  expect(
    cubeOutputCaption({ title: 'My Cube', name: 'My Cube', outputName: 'mask', outputCount: 2 })
      .label,
  ).toBe('My Cube \u00b7 mask');
});

test('a restored workflow’s tabs read as the Cubes’ names, whatever Cubes it holds', () => {
  const sources = presentCanvasSources(
    decodeComfyWorkflowSources(
      workflow([
        { id: 1, title: 'Anima/Text to Image', model: 'Anima', outputs: ['image'] },
        { id: 2, title: 'Anima/Latent Upscale', model: 'Anima', outputs: ['image'] },
        { id: 3, title: 'Mine', model: '', outputs: ['image'] },
      ]),
    ),
  );
  const outputs = sources.filter((source) => source.cube);

  expect(outputs.map((source) => source.label)).toEqual([
    'Text to Image',
    'Latent Upscale',
    'Mine',
  ]);
  // No two tabs collide, so nothing appends a path to them.
  expect(new Set(outputs.map((source) => source.label)).size).toBe(outputs.length);
  expect(outputs[0]?.breadcrumb).toEqual(['Anima/Text to Image', 'image']);
});

test('what a run records about a Cube output is unchanged: the output’s name under its Cube', () => {
  const [, output] = decodeComfyWorkflowSources(
    workflow([{ id: 1, title: 'Anima/Text to Image', model: 'Anima', outputs: ['image'] }]),
  );
  expect(output).toMatchObject({ label: 'image', breadcrumb: ['Anima/Text to Image'] });
});
