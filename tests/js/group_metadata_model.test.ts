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
  normalizeCubeGroupMetadata,
  readCubeDefinitionMetadata,
  readCubeInstanceMetadata,
  readCubePresetMetadata,
  readCubeRuntimeMetadata,
  serializeCubeGroupMetadataForCubeLayout,
  serializeCubeGroupMetadataForWorkflow,
  writeCubeDefinitionMetadata,
  writeCubeInstanceMetadata,
  writeCubePresetMetadata,
  writeCubeRuntimeMetadata,
} from '../../web/comfyui/ui/graph/GroupMetadata.js';

describe('cube group metadata model', () => {
  test('normalizes legacy flat metadata into explicit ownership sections', () => {
    const normalized = normalizeCubeGroupMetadata({
      schema: 5,
      managed: true,
      cube_id: 'local/example-user/demo.cube',
      default_alias: 'Demo',
      target_model: '',
      cube_version: '1.2.3',
      cube_revision_ref: 'current',
      cube_definition_key: 'local/example-user/demo.cube@1.2.3',
      surface_signature: 'surface-1',
      surface: { default_flavor_id: 'default', controls: [] },
      instance_id: 'inst-1',
      instance_alias: 'Demo 2',
      nodes: [1, '2'],
      markers: { inputs: [10], outputs: ['11'] },
      bounds: { x: 1, y: 2, w: 3, h: 4 },
      flavor: 'portrait',
      flavor_scope: 'local',
      active_flavor_values: { 'ksampler.cfg': 7 },
      implementation_dirty: false,
      surface_values_changed: true,
      cosmetic_dirty: true,
      has_saveable_changes: true,
      dirty_at: '2026-01-01T00:00:00Z',
    });

    expect(normalized.definition).toEqual({
      cube_id: 'local/example-user/demo.cube',
      default_alias: 'Demo',
      target_model: '',
      cube_version: '1.2.3',
      cube_revision_ref: 'current',
      cube_definition_key: 'local/example-user/demo.cube@1.2.3',
      surface_signature: 'surface-1',
      surface: { default_flavor_id: 'default', controls: [] },
    });
    expect(normalized.instance).toEqual({
      instance_id: 'inst-1',
      instance_alias: 'Demo 2',
      nodes: ['1', '2'],
      markers: { inputs: ['10'], outputs: ['11'] },
      bounds: { x: 1, y: 2, w: 3, h: 4 },
    });
    expect(normalized.preset).toEqual({
      flavor: 'portrait',
      flavor_scope: 'local',
      active_flavor_values: { 'ksampler.cfg': 7 },
    });
    expect(normalized.runtime).toEqual({
      implementation_dirty: false,
      surface_values_changed: true,
      cosmetic_dirty: true,
      has_saveable_changes: true,
      dirty: false,
      dirty_at: '2026-01-01T00:00:00Z',
    });
  });

  test('reads and writes nested metadata sections', () => {
    const metadata = normalizeCubeGroupMetadata({
      definition: { cube_id: 'local/example-user/demo.cube', default_alias: 'Demo' },
      instance: { instance_id: 'inst-1', instance_alias: 'Demo' },
      preset: { flavor: 'default', flavor_scope: 'authored' },
      runtime: { implementation_dirty: false },
    });

    const withDefinition = writeCubeDefinitionMetadata(metadata, { default_alias: 'Updated' });
    const withInstance = writeCubeInstanceMetadata(withDefinition, { instance_alias: 'Local' });
    const withPreset = writeCubePresetMetadata(withInstance, {
      flavor: 'portrait',
      flavor_scope: 'local',
    });
    const withRuntime = writeCubeRuntimeMetadata(withPreset, { implementation_dirty: true });

    expect(readCubeDefinitionMetadata(withRuntime).default_alias).toBe('Updated');
    expect(readCubeInstanceMetadata(withRuntime).instance_alias).toBe('Local');
    expect(readCubePresetMetadata(withRuntime)).toMatchObject({
      flavor: 'portrait',
      flavor_scope: 'local',
    });
    expect(readCubeRuntimeMetadata(withRuntime).implementation_dirty).toBe(true);
  });

  test('cube layout serialization strips instance preset and runtime fields', () => {
    const layoutMetadata = serializeCubeGroupMetadataForCubeLayout({
      cube_id: 'local/example-user/demo.cube',
      default_alias: 'Demo',
      cube_version: '1.0.0',
      instance_id: 'inst-1',
      instance_alias: 'Demo copy',
      nodes: ['1'],
      markers: { inputs: ['10'] },
      bounds: { x: 0, y: 0, w: 100, h: 100 },
      flavor: 'portrait',
      flavor_scope: 'local',
      active_flavor_values: { 'ksampler.cfg': 8 },
      local_flavors: [{ id: 'portrait' }],
      flavor_options: [{ id: 'portrait' }],
      dirty: true,
      dirty_at: '2026-01-01T00:00:00Z',
      implementation_dirty: true,
      surface_values_changed: true,
      cosmetic_dirty: true,
      has_saveable_changes: true,
    });

    expect(layoutMetadata).toMatchObject({
      cube_id: 'local/example-user/demo.cube',
      default_alias: 'Demo',
      cube_version: '1.0.0',
      nodes: ['1'],
      markers: { inputs: ['10'], outputs: [] },
      bounds: { x: 0, y: 0, w: 100, h: 100 },
    });
    expect(layoutMetadata).not.toHaveProperty('instance_id');
    expect(layoutMetadata).not.toHaveProperty('instance_alias');
    expect(layoutMetadata).not.toHaveProperty('flavor');
    expect(layoutMetadata).not.toHaveProperty('flavor_scope');
    expect(layoutMetadata).not.toHaveProperty('active_flavor_values');
    expect(layoutMetadata).not.toHaveProperty('local_flavors');
    expect(layoutMetadata).not.toHaveProperty('flavor_options');
    expect(layoutMetadata).not.toHaveProperty('dirty');
    expect(layoutMetadata).not.toHaveProperty('dirty_at');
    expect(layoutMetadata).not.toHaveProperty('implementation_dirty');
    expect(layoutMetadata).not.toHaveProperty('surface_values_changed');
    expect(layoutMetadata).not.toHaveProperty('cosmetic_dirty');
    expect(layoutMetadata).not.toHaveProperty('has_saveable_changes');
  });

  test('workflow serialization preserves instance alias and selected flavor', () => {
    const metadata = {
      cube_id: 'local/example-user/demo.cube',
      default_alias: 'Demo',
      instance_id: 'inst-1',
      instance_alias: 'Demo copy',
      flavor: 'portrait',
      flavor_scope: 'local',
      active_flavor_values: { 'ksampler.cfg': 8 },
    };

    expect(serializeCubeGroupMetadataForWorkflow(metadata)).toEqual(metadata);
  });
});
