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
/** Characterize native subgraph serialization for one extracted legacy Cube. */

import { LegacyCubeDefinitionSerializer } from '../../../frontend/comfyui/ui/cube/migration/LegacyCubeDefinitionSerializer.js';
import type { LegacyCubePlan } from '../../../frontend/comfyui/ui/cube/migration/LegacyCubeWorkflowExtractor.js';

describe('LegacyCubeDefinitionSerializer', () => {
  test('preserves real node configuration and replaces marker links with graph I/O', () => {
    const plan: LegacyCubePlan = {
      key: 'instance-a',
      cubeId: 'cube-a',
      cubeVersion: '1.2.3',
      title: 'Legacy Cube',
      metadata: {},
      embeddedSubgraphDefinitions: new Map(),
      position: [100, 200],
      size: [700, 500],
      nodes: [
        {
          id: 11,
          type: 'Processor',
          pos: [300, 350],
          size: [240, 160],
          flags: { collapsed: true },
          order: 3,
          mode: 0,
          inputs: [
            { name: 'image', type: 'IMAGE', link: 101 },
            { name: 'unused', type: 'IMAGE', link: 999 },
            { name: 'setting_a', type: 'INT', link: null, widget: { name: 'setting_a' } },
            { name: 'setting_b', type: 'STRING', link: null, widget: { name: 'setting_b' } },
          ],
          outputs: [{ name: 'IMAGE', type: 'IMAGE', links: [102, 998] }],
          properties: { sugarcubes_symbol: 'processor', custom: 'kept' },
          widgets_values: [42, 'native'],
        },
      ],
      groups: [{ id: 6, title: 'Inner', bounding: [250, 320, 200, 180] }],
      internalLinks: [],
      inputs: [
        {
          markerId: 10,
          name: 'input.image',
          type: 'IMAGE',
          targets: [{ linkId: 101, nodeId: 11, slot: 0 }],
        },
      ],
      outputs: [
        {
          markerId: 12,
          name: 'output.image',
          type: 'IMAGE',
          source: { linkId: 102, nodeId: 11, slot: 0 },
        },
      ],
    };
    let nextId = 0;
    const serializer = new LegacyCubeDefinitionSerializer(() => `io-${(nextId += 1)}`);

    const definition = serializer.serialize(plan, 'definition-a');

    expect(definition).toMatchObject({
      id: 'definition-a',
      name: 'Cube: Legacy Cube',
      version: 1,
      inputs: [
        {
          id: 'io-1',
          name: 'input.image',
          localized_name: 'input.image',
          type: 'IMAGE',
          linkIds: [101],
        },
      ],
      outputs: [
        {
          id: 'io-2',
          name: 'output.image',
          localized_name: 'output.image',
          type: 'IMAGE',
          linkIds: [102],
        },
      ],
      links: [
        {
          id: 101,
          origin_id: -10,
          origin_slot: 0,
          target_id: 11,
          target_slot: 0,
          type: 'IMAGE',
        },
        {
          id: 102,
          origin_id: 11,
          origin_slot: 0,
          target_id: -20,
          target_slot: 0,
          type: 'IMAGE',
        },
      ],
    });
    expect(definition.nodes).toEqual([
      expect.objectContaining({
        id: 11,
        type: 'Processor',
        pos: [200, 150],
        flags: { collapsed: true },
        properties: { sugarcubes_symbol: 'processor', custom: 'kept' },
        widgets_values: [42, 'native'],
        inputs: [
          { name: 'image', type: 'IMAGE', link: 101 },
          { name: 'unused', type: 'IMAGE', link: null },
          { name: 'setting_a', type: 'INT', link: null, widget: { name: 'setting_a' } },
          { name: 'setting_b', type: 'STRING', link: null, widget: { name: 'setting_b' } },
        ],
        outputs: [{ name: 'IMAGE', type: 'IMAGE', links: [102] }],
      }),
    ]);
    expect(definition.groups).toEqual([{ id: 6, title: 'Inner', bounding: [150, 120, 200, 180] }]);
    expect(
      (definition.nodes as Array<{ type: string }>).some((node) =>
        node.type.startsWith('SugarCubes.Cube'),
      ),
    ).toBe(false);
  });

  test('recovers the exact 047 KSampler values from its saved instance definition clone', () => {
    const cloneId = 'instance-specific-subgraph';
    const plan = createPlan({
      metadata: {
        implementation_dirty: false,
        surface_values_changed: false,
        has_saveable_changes: false,
      },
      node: {
        id: 11,
        type: cloneId,
        pos: [100, 200],
        properties: {
          sugarcubes_symbol: 'ksampler',
          sugarcubes_original_subgraph_id: 'shared-subgraph',
        },
        widgets_values: [9508555241579063000, 30, 5, 'euler_ancestral', 'normal', 2],
      },
      definitions: new Map([[cloneId, kSamplerCloneDefinition(cloneId)]]),
    });
    const serializer = new LegacyCubeDefinitionSerializer(() => 'boundary-id');

    const definition = serializer.serialize(plan, 'definition-a');

    expect(definition.nodes[0]?.widgets_values).toEqual([
      1080,
      1512,
      9508555241579063000,
      30,
      5,
      'euler_ancestral',
      'normal',
      2,
    ]);
  });

  test('refuses to guess how edited Sugar-DSL wrapper values map to current widgets', () => {
    const plan = createPlan({
      metadata: { surface_values_changed: true },
      node: {
        id: 11,
        type: 'instance-specific-subgraph',
        pos: [100, 200],
        properties: { sugarcubes_original_subgraph_id: 'shared-subgraph' },
        widgets_values: [123456, 30, 5, 'euler_ancestral', 'normal'],
      },
      definitions: new Map([
        ['instance-specific-subgraph', kSamplerCloneDefinition('instance-specific-subgraph')],
      ]),
    });
    const serializer = new LegacyCubeDefinitionSerializer(() => 'boundary-id');

    expect(() => serializer.serialize(plan, 'definition-a')).toThrow(
      'contains edited positional widget values that cannot be restored safely',
    );
  });

  test('fails closed when a direct legacy node has values but no saved widget identities', () => {
    const plan = createPlan({
      metadata: {},
      node: {
        id: 2,
        type: 'SimpleSyrup.SimpleLoadAnima',
        properties: { sugarcubes_symbol: 'models' },
        inputs: [],
        widgets_values: ['Anima\\model.safetensors', 'default', 'auto', 'default', 'auto'],
      },
    });
    const serializer = new LegacyCubeDefinitionSerializer(() => 'boundary-id');

    expect(() => serializer.serialize(plan, 'definition-a')).toThrow(
      "Node 'models' contains positional widget values without same-snapshot identities.",
    );
  });
});

/** Build the smallest detached plan needed to characterize nested wrapper restoration. */
function createPlan({
  metadata,
  node,
  definitions = new Map(),
}: {
  metadata: LegacyCubePlan['metadata'];
  node: LegacyCubePlan['nodes'][number];
  definitions?: ReadonlyMap<string, LegacyCubePlan['nodes'][number]>;
}): LegacyCubePlan {
  return {
    key: 'instance-a',
    cubeId: 'cube-a',
    cubeVersion: '1.0.0',
    title: 'Legacy Cube',
    metadata,
    embeddedSubgraphDefinitions: definitions,
    position: [100, 200],
    size: [700, 500],
    nodes: [node],
    groups: [],
    internalLinks: [],
    inputs: [],
    outputs: [],
  };
}

/** Build the compact co-persisted clone that owns the reported 047 values. */
function kSamplerCloneDefinition(id: string): LegacyCubePlan['nodes'][number] {
  return {
    id,
    inputs: [
      { name: 'model', linkIds: [1] },
      { name: 'positive', linkIds: [2] },
      { name: 'negative', linkIds: [3] },
      { name: 'width', linkIds: [4] },
      { name: 'height', linkIds: [5] },
      { name: 'seed', linkIds: [6] },
      { name: 'steps', linkIds: [7] },
      { name: 'cfg', linkIds: [8] },
      { name: 'sampler_name', linkIds: [9] },
      { name: 'scheduler', linkIds: [10] },
      { name: 'batch_size', linkIds: [11] },
    ],
    nodes: [
      {
        id: 101,
        inputs: [
          { name: 'width', widget: { name: 'width' }, link: 4 },
          { name: 'height', widget: { name: 'height' }, link: 5 },
          { name: 'batch_size', widget: { name: 'batch_size' }, link: 11 },
        ],
        widgets_values: [1080, 1512, 2],
      },
      {
        id: 102,
        inputs: [
          { name: 'model', link: 1 },
          { name: 'positive', link: 2 },
          { name: 'negative', link: 3 },
          { name: 'latent_image', link: 12 },
          { name: 'seed', widget: { name: 'seed' }, link: 6 },
          { name: 'steps', widget: { name: 'steps' }, link: 7 },
          { name: 'cfg', widget: { name: 'cfg' }, link: 8 },
          { name: 'sampler_name', widget: { name: 'sampler_name' }, link: 9 },
          { name: 'scheduler', widget: { name: 'scheduler' }, link: 10 },
          { name: 'denoise', widget: { name: 'denoise' }, link: null },
        ],
        widgets_values: [9508555241579063000, 'randomize', 30, 5, 'euler_ancestral', 'normal', 1],
      },
    ],
    links: [
      { id: 1, origin_id: -10, target_id: 102, target_slot: 0 },
      { id: 2, origin_id: -10, target_id: 102, target_slot: 1 },
      { id: 3, origin_id: -10, target_id: 102, target_slot: 2 },
      { id: 4, origin_id: -10, target_id: 101, target_slot: 0 },
      { id: 5, origin_id: -10, target_id: 101, target_slot: 1 },
      { id: 6, origin_id: -10, target_id: 102, target_slot: 4 },
      { id: 7, origin_id: -10, target_id: 102, target_slot: 5 },
      { id: 8, origin_id: -10, target_id: 102, target_slot: 6 },
      { id: 9, origin_id: -10, target_id: 102, target_slot: 7 },
      { id: 10, origin_id: -10, target_id: 102, target_slot: 8 },
      { id: 11, origin_id: -10, target_id: 101, target_slot: 2 },
    ],
    extra: { sugar: { original_subgraph_id: 'shared-subgraph' } },
  };
}
