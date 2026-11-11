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
/** Safeguard explicit cube-family ownership without deriving compatibility from filenames. */
import { nativeModelFieldFixture } from '../support/native_model_field.js';
import { readComfyModelFields } from '../../../frontend/comfyui/ui/model_library/ComfyModelFieldAdapter.js';
import { comfyModelTarget } from '../../../frontend/comfyui/ui/model_library/ComfyModelDiscoveryContext.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import type { NativeCubeSubgraph } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';

/** Describe a graph-scoped native node without importing the live Comfy singleton. */
function cube(graphId: string, target: string): CubeNode {
  const subgraph: NativeCubeSubgraph = {
    id: graphId,
    name: 'Cube definition',
    _nodes: [],
    inputs: [],
    outputs: [],
    inputNode: {},
    outputNode: {},
    add: () => undefined,
    remove: () => undefined,
    addInput: () => {
      throw new Error('Unused fixture input');
    },
    addOutput: () => {
      throw new Error('Unused fixture output');
    },
    configure: () => undefined,
  };
  subgraph._nodes.push({
    id: 10,
    pos: [0, 0],
    size: [100, 100],
    properties: {},
    inputs: [],
    outputs: [],
    connect: () => undefined,
  });
  return {
    id: graphId + '-instance',
    type: graphId,
    title: 'Cube',
    pos: [0, 0],
    size: [500, 500],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { target_model: target },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph,
    isSubgraphNode: () => true,
    serialize: () => ({}),
    connect: () => undefined,
  };
}

afterEach(() => document.body.replaceChildren());

test('requires exact graph scope and prefers explicit instance family metadata', () => {
  const native = nativeModelFieldFixture();
  const binding = readComfyModelFields(native.card, native.snapshot)[0];
  if (!binding) throw new Error('Missing native model field.');
  const owner = cube('sdxl-definition', 'SDXL');
  owner.subgraph.extra = { sugarcubes_cube: { target_model: 'Flux' } };
  const other = cube('flux-definition', 'Flux');
  expect(comfyModelTarget(binding, [owner, other])).toBe('');
  native.card.dataset.cubeNodeLocator = 'sdxl-definition:10';
  expect(comfyModelTarget(binding, [owner, other])).toBe('SDXL');
  for (const kind of ['upscale_models', 'vae', 'embeddings', 'text_encoders', 'unet', 'misc'])
    expect(comfyModelTarget({ ...binding, kind }, [owner, other])).toBe('');
  expect(comfyModelTarget({ ...binding, nodeLocator: 'flux-definition:10' }, [owner, other])).toBe(
    'Flux',
  );
  expect(comfyModelTarget({ ...binding, nodeLocator: '10' }, [owner, other])).toBe('');
  expect(comfyModelTarget(binding, [owner, cube('sdxl-definition', 'Flux')])).toBe('');
  owner.properties.sugarcubes_cube = {};
  expect(comfyModelTarget(binding, [owner])).toBe('Flux');
});

test('follows nested native cube scopes and terminates cycles without assigning ambiguous ownership', () => {
  const native = nativeModelFieldFixture();
  const binding = readComfyModelFields(native.card, native.snapshot)[0];
  if (!binding) throw new Error('Missing model field.');
  const owner = cube('outer', 'Anima');
  const nested = cube('nested', 'SDXL');
  owner.subgraph._nodes.push(nested);
  nested.subgraph._nodes.push(owner);
  expect(comfyModelTarget({ ...binding, nodeLocator: 'nested:10' }, [owner])).toBe('Anima');
  expect(comfyModelTarget({ ...binding, nodeLocator: 'missing:10' }, [owner])).toBe('');
  expect(comfyModelTarget({ ...binding, nodeLocator: 'nested:10' }, [owner, nested])).toBe('');
});
