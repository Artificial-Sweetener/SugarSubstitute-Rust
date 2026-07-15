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
import { describe, expect, test } from '@jest/globals';
import {
  allocateUniqueInstanceAlias,
  syncInstanceAlias,
} from '../../web/comfyui/ui/graph/InstanceAliasSync.js';

function makeGroup({ name, instanceId }: { name?: string; instanceId?: string } = {}) {
  return {
    title: name,
    properties: {
      sugarcubes: {
        managed: true,
        default_alias: name,
        instance_alias: name,
        instance_id: instanceId || '',
      },
    },
  };
}

describe('allocateUniqueInstanceAlias', () => {
  test('returns desired name when free', () => {
    const graph = { _groups: [makeGroup({ name: 'Alpha', instanceId: 'inst-1' })] };
    const result = allocateUniqueInstanceAlias(graph, 'Beta', { currentInstanceId: 'inst-2' });
    expect(result).toBe('Beta');
  });

  test('suffixes when taken (case-insensitive)', () => {
    const graph = { _groups: [makeGroup({ name: 'Alpha', instanceId: 'inst-1' })] };
    const result = allocateUniqueInstanceAlias(graph, 'alpha', { currentInstanceId: 'inst-2' });
    expect(result).toBe('alpha 2');
  });

  test('allows same instance to keep name', () => {
    const graph = { _groups: [makeGroup({ name: 'Alpha', instanceId: 'inst-1' })] };
    const result = allocateUniqueInstanceAlias(graph, 'Alpha', { currentInstanceId: 'inst-1' });
    expect(result).toBe('Alpha');
  });
});

describe('syncInstanceAlias', () => {
  test('updates alias for markers scoped to metadata markers before cube_id', () => {
    const markerA = {
      id: 1,
      type: 'SugarCubes.CubeOutput',
      widgets: [
        { name: 'cube_id', value: 'local/demo' },
        { name: 'default_alias', value: 'Demo' },
        { name: 'instance_alias', value: 'Demo' },
      ],
    };
    const markerB = {
      id: 2,
      type: 'SugarCubes.CubeOutput',
      widgets: [
        { name: 'cube_id', value: 'local/demo' },
        { name: 'default_alias', value: 'Demo' },
        { name: 'instance_alias', value: 'Demo' },
      ],
    };
    const graph = { _nodes: [markerA, markerB] };
    const metadata = {
      default_alias: 'Demo',
      instance_alias: 'Demo',
      markers: { inputs: [], outputs: [1] },
    };

    syncInstanceAlias({
      graph,
      group: null,
      metadata,
      cubeId: 'local/demo',
      instanceAlias: 'Demo 2',
    });

    expect(metadata.instance_alias).toBe('Demo 2');
    expect(markerA.widgets.find((w) => w.name === 'instance_alias')?.value).toBe('Demo 2');
    expect(markerB.widgets.find((w) => w.name === 'instance_alias')?.value).toBe('Demo');
    expect(markerA.widgets.find((w) => w.name === 'default_alias')?.value).toBe('Demo');
  });
});
