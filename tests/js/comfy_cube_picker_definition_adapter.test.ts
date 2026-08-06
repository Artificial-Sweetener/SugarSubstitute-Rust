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
/** Verify Comfy definition contribution, ordering, and dynamic reconciliation. */

import { describe, expect, jest, test } from '@jest/globals';
import { ComfyCubePickerDefinitionAdapter } from '../../frontend/comfyui/ui/picker/ComfyCubePickerDefinitionAdapter.js';
import type { CubePickerCatalogRegistry } from '../../frontend/comfyui/ui/picker/CubePickerCatalogRegistry.js';
import type { ComfyCubeNodeDefinition } from '../../frontend/comfyui/ui/picker/ComfyCubeNodeDefProjector.js';
import type { UnknownRecord } from '../../frontend/comfyui/ui/types/common.js';

const typeA = `SugarCubes.Cube.${'a'.repeat(64)}`;
const typeB = `SugarCubes.Cube.${'b'.repeat(64)}`;

describe('ComfyCubePickerDefinitionAdapter', () => {
  test('contributes only placement-ready definitions and restores them on Vue refresh', async () => {
    let current = [definition(typeA, 'Cube A')];
    const refresh = jest.fn(async () => ({
      changed: true,
      revision: 'one',
      definitions: current,
      unavailableCubeIds: [],
    }));
    const adapter = new ComfyCubePickerDefinitionAdapter({
      registry: {
        refresh,
        definitions: () => current,
      } as unknown as CubePickerCatalogRegistry,
      app: {},
      liteGraph: {},
    });
    const startupDefinitions: Record<string, UnknownRecord> = {
      Ordinary: { name: 'Ordinary' },
    };

    await adapter.contribute(startupDefinitions);
    expect(startupDefinitions[typeA]?.category).toBe('SugarCubes');

    current = [definition(typeB, 'Cube B')];
    const vueDefinitions: UnknownRecord[] = [
      { name: 'Ordinary' },
      { name: typeA, category: 'SugarCubes' },
    ];
    adapter.orderForVue(vueDefinitions);
    expect(vueDefinitions.map(({ name }) => name)).toEqual([typeB, 'Ordinary']);
  });

  test('keeps Sugar first among contributed definitions so Comfy Blueprints prepend adjacently', () => {
    const projected = [definition(typeA, 'Cube A')];
    const adapter = new ComfyCubePickerDefinitionAdapter({
      registry: {
        definitions: () => projected,
      } as unknown as CubePickerCatalogRegistry,
      app: {},
      liteGraph: {},
    });
    const definitions: UnknownRecord[] = [
      { name: 'KSampler', category: 'sampling' },
      { name: 'LoadImage', category: 'image' },
    ];

    adapter.orderForVue(definitions);
    definitions.unshift({ name: 'SubgraphBlueprint.demo', category: 'Subgraph Blueprints' });

    expect(definitions.map(({ category }) => category)).toEqual([
      'Subgraph Blueprints',
      'SugarCubes',
      'sampling',
      'image',
    ]);
  });

  test('registers updates, unregisters removals, and refreshes Vue consumers', async () => {
    let current = [definition(typeA, 'Cube A')];
    const refresh = jest.fn(async () => ({
      changed: true,
      revision: 'next',
      definitions: current,
      unavailableCubeIds: [],
    }));
    const registerNodeDef = jest.fn(async () => undefined);
    const reloadNodeDefs = jest.fn(async () => undefined);
    const unregisterNodeType = jest.fn();
    const adapter = new ComfyCubePickerDefinitionAdapter({
      registry: {
        refresh,
        definitions: () => current,
      } as unknown as CubePickerCatalogRegistry,
      app: { registerNodeDef, reloadNodeDefs },
      liteGraph: { unregisterNodeType },
    });
    await adapter.contribute({});
    current = [definition(typeB, 'Cube B')];

    await adapter.reconcile();

    expect(refresh).toHaveBeenLastCalledWith({ force: true });
    expect(unregisterNodeType).toHaveBeenCalledWith(typeA);
    expect(registerNodeDef).toHaveBeenCalledWith(
      typeB,
      expect.objectContaining({ name: typeB, display_name: 'Cube B' }),
    );
    expect(reloadNodeDefs).toHaveBeenCalledTimes(1);
  });

  test('fails closed when dynamic host registration is unavailable', async () => {
    const projected = [definition(typeA, 'Cube A')];
    const adapter = new ComfyCubePickerDefinitionAdapter({
      registry: {
        refresh: async () => ({
          changed: true,
          revision: 'one',
          definitions: projected,
          unavailableCubeIds: [],
        }),
        definitions: () => projected,
      } as unknown as CubePickerCatalogRegistry,
      app: {},
      liteGraph: {},
    });

    await expect(adapter.reconcile()).rejects.toThrow('registration is unavailable');
  });
});

/** Build one complete V1 definition for the host adapter boundary. */
function definition(name: string, displayName: string): ComfyCubeNodeDefinition {
  return {
    name,
    display_name: displayName,
    description: '',
    category: 'SugarCubes',
    python_module: 'custom_nodes.SugarCubes',
    output_node: false,
    input: { required: {} },
    input_order: { required: [] },
    output: [],
    output_name: [],
    output_is_list: [],
    search_aliases: [],
  };
}
