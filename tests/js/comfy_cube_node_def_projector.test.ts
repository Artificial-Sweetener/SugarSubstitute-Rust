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
import {
  CUBE_NODE_TYPE_PREFIX,
  projectComfyCubeNodeDef,
} from '../../frontend/comfyui/ui/picker/ComfyCubeNodeDefProjector.js';
import {
  readCubePickerCatalog,
  type CubePickerDescriptor,
} from '../../frontend/comfyui/ui/picker/CubePickerDescriptor.js';

const KEY = '4'.repeat(64);

function descriptor(overrides: Partial<CubePickerDescriptor> = {}): CubePickerDescriptor {
  return {
    key: KEY,
    cubeId: 'owner/repo/demo.cube',
    version: '1.0.0',
    displayName: 'Demo',
    description: 'Demo Cube',
    searchTerms: ['owner/repo/demo.cube', 'portrait'],
    targetModel: 'SDXL',
    supportedModels: ['SDXL'],
    requiredCustomNodes: [],
    source: { kind: 'github', repoRef: 'owner/repo', path: 'demo.cube' },
    inputs: [{ id: 'seed', name: 'seed', label: 'Seed', type: 'INT' }],
    outputs: [{ id: 'image', name: 'image', label: 'Image', type: 'IMAGE' }],
    ...overrides,
  };
}

describe('Comfy Cube node-definition projection', () => {
  test('advertises scalar boundaries as socket-only required inputs', () => {
    expect(projectComfyCubeNodeDef(descriptor())).toEqual({
      name: `${CUBE_NODE_TYPE_PREFIX}${KEY}`,
      display_name: 'Demo',
      description: 'Demo Cube',
      category: 'SugarCubes/SDXL',
      python_module: 'custom_nodes.repo',
      sugarcubes_pack_name: 'repo',
      sugarcubes_target_model: 'SDXL',
      output_node: false,
      input: {
        required: {
          seed: ['INT', { forceInput: true, display_name: 'Seed' }],
        },
      },
      input_order: { required: ['seed'] },
      output: ['IMAGE'],
      output_name: ['Image'],
      output_is_list: [false],
      search_aliases: ['owner/repo/demo.cube', 'portrait'],
    });
  });

  test('advertises no interface for a control-rich zero-boundary descriptor', () => {
    const definition = projectComfyCubeNodeDef(descriptor({ inputs: [], outputs: [] }));

    expect(definition.input.required).toEqual({});
    expect(definition.output).toEqual([]);
    expect(definition.output_name).toEqual([]);
  });

  test('keeps legacy descriptors discoverable under an explicit model fallback', () => {
    const definition = projectComfyCubeNodeDef(descriptor({ targetModel: '' }));

    expect(definition.category).toBe('SugarCubes/Unspecified');
  });

  test('rejects incompatible schemas and duplicate boundary names', () => {
    const rawDescriptor = descriptor({
      inputs: [
        { id: 'one', name: 'image', label: 'First', type: 'IMAGE' },
        { id: 'two', name: 'image', label: 'Second', type: 'IMAGE' },
      ],
    });

    expect(
      readCubePickerCatalog({
        schemaVersion: 2,
        catalogRevision: 'sha256:test',
        entries: [],
        errors: [],
      }),
    ).toBeNull();
    expect(
      readCubePickerCatalog({
        schemaVersion: 1,
        catalogRevision: 'sha256:test',
        entries: [rawDescriptor],
        errors: [],
      }),
    ).toBeNull();
    expect(
      readCubePickerCatalog({
        schemaVersion: 1,
        catalogRevision: 'sha256:test',
        entries: [descriptor(), descriptor({ cubeId: 'different.cube' })],
        errors: [],
      }),
    ).toBeNull();
  });
});
