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
/** Verify Add Cube discovery follows boundary and model compatibility policy. */

import { expect, jest, test } from '@jest/globals';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeAddCandidateCatalog } from '../../../frontend/comfyui/ui/picker/CubeAddCandidateCatalog.js';
import type { CubePickerCatalogEntry } from '../../../frontend/comfyui/ui/picker/CubePickerCatalogRegistry.js';

test('groups only exact target-family Cubes first and searches other models afterward', () => {
  const catalog = new CubeAddCandidateCatalog({
    compatibility: { accepts: (output, input) => output === input },
    strict: () => false,
  });
  const groups = catalog.candidates(sourceCube(), [
    entry('sdxl-upscale', 'SDXL/Diffusion Upscale', 'SDXL', ['SDXL'], ['IMAGE']),
    entry('flux-upscale', 'Flux/Diffusion Upscale', 'Flux', ['Flux', 'SDXL'], ['IMAGE']),
    entry('anima-upscale', 'Anima/Diffusion Upscale', 'Anima', ['Anima'], ['IMAGE']),
    entry('text-to-image', 'Text to Image', 'SDXL', ['SDXL'], []),
    entry('mask-tool', 'Mask Tool', 'SDXL', ['SDXL'], ['MASK']),
  ]);

  expect(groups.sameModel.map(({ displayName }) => displayName)).toEqual([
    'SDXL/Diffusion Upscale',
  ]);
  expect(groups.otherModels.map(({ displayName }) => displayName)).toEqual([
    'Anima/Diffusion Upscale',
    'Flux/Diffusion Upscale',
  ]);
  expect(catalog.search(groups, 'upscale').map(({ displayName }) => displayName)).toEqual([
    'SDXL/Diffusion Upscale',
    'Anima/Diffusion Upscale',
    'Flux/Diffusion Upscale',
  ]);
  expect(catalog.search(groups, 'anima').map(({ displayName }) => displayName)).toEqual([
    'Anima/Diffusion Upscale',
  ]);
});

test('does not advertise proximity choices from a hard-linked source output', () => {
  const source = sourceCube();
  source.outputs[0]!.links = [99];
  const groups = new CubeAddCandidateCatalog({
    compatibility: { accepts: () => true },
    strict: () => false,
  }).candidates(source, [entry('upscale', 'Upscale', 'SDXL', ['SDXL'], ['IMAGE'])]);

  expect(groups).toEqual({ sameModel: [], otherModels: [] });
});

test('treats an Any-model Cube as model compatible when its boundary matches', () => {
  const groups = new CubeAddCandidateCatalog({
    compatibility: { accepts: (output, input) => output === input },
    strict: () => true,
  }).candidates(sourceCube(), [entry('any-upscale', 'Any Upscale', 'Any', [], ['IMAGE'])]);

  expect(groups.sameModel.map(({ displayName }) => displayName)).toEqual(['Any Upscale']);
  expect(groups.otherModels).toEqual([]);
});

test('evaluates strictness once and reuses identical compatibility decisions', () => {
  const accepts = jest.fn((output: unknown, input: unknown) => output === input);
  const strict = jest.fn(() => true);
  const source = sourceCube();
  source.outputs.push({ name: 'duplicate image', type: 'IMAGE', links: null });
  const entries = Array.from({ length: 250 }, (_, index) =>
    entry(
      `upscale-${String(index)}`,
      `SDXL/Upscale ${String(index)}`,
      'SDXL',
      ['SDXL'],
      ['IMAGE', 'IMAGE'],
    ),
  );

  const groups = new CubeAddCandidateCatalog({
    compatibility: { accepts },
    strict,
  }).candidates(source, entries);

  expect(groups.sameModel).toHaveLength(250);
  expect(strict).toHaveBeenCalledTimes(1);
  expect(accepts).toHaveBeenCalledTimes(1);
  expect(accepts).toHaveBeenCalledWith('IMAGE', 'IMAGE', true);
});

/** Build one source Cube with a free IMAGE proximity output. */
function sourceCube(): CubeNode {
  return {
    id: 'source',
    type: 'source',
    title: 'Text to Image',
    pos: [0, 0],
    size: [240, 160],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        instance_id: 'source',
        target_model: 'SDXL',
        supported_models: ['SDXL', 'SD 1.5'],
      },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [{ name: 'image', type: 'IMAGE', links: null }],
    subgraph: {
      id: 'source-definition',
      name: 'Text to Image',
      _nodes: [],
      inputs: [],
      outputs: [],
      inputNode: {},
      outputNode: {},
      add() {},
      remove() {},
      addInput() {
        throw new Error('not used');
      },
      addOutput() {
        throw new Error('not used');
      },
      configure() {},
    },
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}

/** Build one validated picker entry with public input types. */
function entry(
  type: string,
  displayName: string,
  targetModel: string,
  supportedModels: string[],
  inputTypes: string[],
): CubePickerCatalogEntry {
  return {
    type,
    descriptor: {
      key: type.padEnd(64, 'a').slice(0, 64),
      cubeId: `local/${type}.cube`,
      version: '1.0.0',
      displayName,
      description: `${displayName} description`,
      searchTerms: [displayName, targetModel],
      targetModel,
      supportedModels,
      requiredCustomNodes: [],
      source: {},
      inputs: inputTypes.map((inputType, index) => ({
        id: `input-${String(index)}`,
        name: `input.${String(index)}`,
        label: inputType,
        type: inputType,
      })),
      outputs: [],
    },
  };
}
