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
/** Prove whole-node sharing matches stable symbols and connection shapes across native instances. */
import { expect, test } from '@jest/globals';
import { NativeRelationIdentity } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationIdentity.js';
import type { ComfyGraph, ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

/** Supply a resource patch whose upstream node IDs vary while authored symbols remain stable. */
function patch(id: number, sourceSymbol = 'provider'): ComfyNode {
  const source: ComfyNode = { id: id + 1, properties: { sugarcubes_symbol: sourceSymbol } };
  const graph: ComfyGraph = {
    _nodes: [source],
    links: new Map([
      [id, { id, origin_id: source.id, origin_slot: 0, target_id: id, target_slot: 0 }],
    ]),
  };
  const node: ComfyNode = { id, graph, inputs: [{ name: 'model', link: id }], properties: {} };
  graph._nodes!.push(node);
  return node;
}

test('matches stable connection symbols across different numeric IDs and rejects shape differences', () => {
  const index = new NativeRelationIdentity();
  expect(index.read(patch(1), 'vectorscope', ['brightness'])).toBe(
    index.read(patch(10), 'vectorscope', ['brightness']),
  );
  expect(index.read(patch(1), 'vectorscope', ['brightness'])).not.toBe(
    index.read(patch(10, 'other-provider'), 'vectorscope', ['brightness']),
  );
  const unresolved = patch(20);
  unresolved.graph!._nodes = [unresolved];
  expect(index.read(unresolved, 'vectorscope', ['brightness'])).toBeNull();
});

test('excludes editable values from identity and rejects unresolved connected input contracts', () => {
  const index = new NativeRelationIdentity();
  const node = patch(1);
  node.inputs!.push({ name: 'brightness', link: 99 });
  expect(index.read(node, 'vectorscope', ['brightness'])).not.toBeNull();
  expect(index.read(node, 'vectorscope', [])).toBeNull();
  expect(index.read({ inputs: [{ name: 'model', link: 0 }] }, 'vectorscope', [])).toBeNull();
});
