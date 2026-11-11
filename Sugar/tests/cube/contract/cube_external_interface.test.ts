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
/** Verify authored native boundaries become public only when wired internally. */

import { resolveCubeExternalInterface } from '../../../frontend/comfyui/ui/cube/graph/CubeExternalInterface.js';

describe('resolveCubeExternalInterface', () => {
  test('keeps dormant empty-draft boundaries inside the editor', () => {
    const interfaceState = resolveCubeExternalInterface({
      inputs: [{ name: 'input' }, { name: 'second input' }],
      outputs: [{ name: 'output' }, { name: 'second output' }],
      subgraph: {
        inputNode: { slots: [{ linkIds: [] }, { linkIds: [101] }] },
        outputNode: { slots: [{ linkIds: [] }, { linkIds: [202] }] },
      },
    });

    expect(interfaceState).toEqual({ inputSlots: [1], outputSlots: [1] });
  });

  test('retains ports when an older host does not expose native boundary slots', () => {
    const interfaceState = resolveCubeExternalInterface({
      inputs: [{ name: 'input' }],
      outputs: [{ name: 'output' }],
      subgraph: { inputNode: {}, outputNode: {} },
    });

    expect(interfaceState).toEqual({ inputSlots: [0], outputSlots: [0] });
  });

  test('does not publish uninspectable draft placeholders on the Cube face', () => {
    const interfaceState = resolveCubeExternalInterface({
      inputs: [{ name: 'input' }],
      outputs: [{ name: 'output' }],
      properties: { sugarcubes_kind: 'cube_draft' },
      subgraph: { inputNode: {}, outputNode: {} },
    });

    expect(interfaceState).toEqual({ inputSlots: [], outputSlots: [] });
  });

  test('recognizes a draft marker restored from the native subgraph document', () => {
    const interfaceState = resolveCubeExternalInterface({
      inputs: [{ name: 'input' }],
      outputs: [{ name: 'output' }],
      subgraph: {
        extra: { sugarcubes_kind: 'cube_draft' },
        inputNode: {},
        outputNode: {},
      },
    });

    expect(interfaceState).toEqual({ inputSlots: [], outputSlots: [] });
  });

  test('recognizes the draft identity used by the Cube action menu', () => {
    const interfaceState = resolveCubeExternalInterface({
      inputs: [{ name: 'input' }],
      outputs: [{ name: 'output' }],
      properties: { sugarcubes_cube: { kind: 'draft' } },
      subgraph: { inputNode: {}, outputNode: {} },
    });

    expect(interfaceState).toEqual({ inputSlots: [], outputSlots: [] });
  });

  test('recognizes the embedded draft identity restored from a workflow', () => {
    const interfaceState = resolveCubeExternalInterface({
      inputs: [{ name: 'input' }],
      outputs: [{ name: 'output' }],
      subgraph: {
        extra: { sugarcubes_cube: { kind: 'draft' } },
        inputNode: {},
        outputNode: {},
      },
    });

    expect(interfaceState).toEqual({ inputSlots: [], outputSlots: [] });
  });

  test('prefers Comfy native boundary resolution over serialized placeholder links', () => {
    const interfaceState = resolveCubeExternalInterface({
      inputs: [{ name: 'input' }],
      outputs: [{ name: 'output' }],
      properties: { sugarcubes_cube: { kind: 'draft' } },
      subgraph: {
        inputNode: { slots: [{ linkIds: [1] }] },
        outputNode: { slots: [{ linkIds: [2] }] },
      },
      resolveSubgraphInputLinks: () => [],
      resolveSubgraphOutputLink: () => undefined,
    });

    expect(interfaceState).toEqual({ inputSlots: [], outputSlots: [] });
  });
});
