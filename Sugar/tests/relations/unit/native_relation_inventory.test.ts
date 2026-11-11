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
/** Prove root exposure and graph connection exclusions independently from rendering. */
import { expect, test } from '@jest/globals';
import { NativeRelationInventory } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationInventory.js';
import { parseRelationState } from '../../../frontend/comfyui/ui/relations/infrastructure/RelationWorkflowMetadata.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

/** Supply a seed capability with an actual native numeric widget contract. */
function sampler(id: string): ComfyNode {
  return {
    id,
    type: 'KSampler',
    widgets: [
      { name: 'seed', type: 'number', value: 42, options: { min: 0, max: 100, precision: 0 } },
    ],
  };
}

test('includes root editors but never scans hidden ordinary subgraph contents', () => {
  const inventory = new NativeRelationInventory(console);
  const hidden = sampler('hidden');
  const outer: ComfyNode = {
    id: 'outer',
    type: 'Subgraph',
    widgets: [],
    isSubgraphNode: () => true,
    subgraph: { _nodes: [hidden] },
  };
  expect(
    inventory
      .read({ _nodes: [sampler('root'), outer] })
      .fields.map((field) => field.contract.semantic),
  ).toEqual(['seed']);
});

test('an upstream-connected widget contributes neither membership nor restrictions', () => {
  const connected = sampler('connected');
  connected.inputs = [{ name: 'seed', widget: { name: 'seed' }, link: 0 }];
  const source = sampler('source');
  const inventory = new NativeRelationInventory(console).read({ _nodes: [connected, source] });
  expect(inventory.fields).toHaveLength(1);
  expect(inventory.fields[0]?.contract.nodeId).toContain('source');
});

test('keeps collapsed advanced capabilities and rejects malformed persisted state', () => {
  const node = sampler('root');
  node.flags = { collapsed: true };
  node.widgets![0]!.options!.advanced = true;
  expect(new NativeRelationInventory(console).read({ _nodes: [node] }).fields).toHaveLength(1);
  expect(
    parseRelationState({
      schema: 1,
      overrides: { seed: { enabled: true, value: [] } },
      fieldLinks: { a: 12, b: 'a' },
    }),
  ).toMatchObject({ overrides: {}, fieldLinks: { b: 'a' } });
});

test('intersects every exposed native subgraph target while keeping hidden widgets excluded', () => {
  const left = sampler('left'),
    right = sampler('right');
  left.inputs = [{ name: 'seed', widget: { name: 'seed' } }];
  right.inputs = [{ name: 'seed', widget: { name: 'seed' } }];
  right.widgets![0]!.options!.max = 20;
  const facade: ComfyNode = {
    id: 'face',
    type: 'Subgraph',
    isSubgraphNode: () => true,
    subgraph: { _nodes: [left, right, sampler('hidden')] },
    inputs: [{ name: 'exposedSeed', widget: { name: 'exposedSeed' } }],
    widgets: [
      {
        name: 'exposedSeed',
        type: 'number',
        value: 10,
        options: { min: 0, max: 100, precision: 0 },
      },
    ],
    resolveSubgraphInputLinks: () =>
      [left, right].map((node) => ({
        inputNode: node,
        input: node.inputs?.[0],
        link: { target_slot: 0 },
      })),
  };
  const inventory = new NativeRelationInventory(console).read({ _nodes: [facade] });
  expect(inventory.fields).toHaveLength(1);
  expect(inventory.fields[0]?.contract).toMatchObject({
    semantic: 'seed',
    name: 'exposedSeed',
    domain: { maximum: 20 },
  });
  facade.inputs![0]!.link = 1;
  expect(new NativeRelationInventory(console).read({ _nodes: [facade] }).fields).toHaveLength(0);
});
