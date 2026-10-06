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
/** Safeguard flattened prompt identities while following promoted input ownership. */
import { jest } from '@jest/globals';
import { NativeSubgraphBoundaryResolver } from '../../../frontend/comfyui/ui/cube/graph/NativeSubgraphBoundaryResolver.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

/** Build one real leaf input whose public name differs from the containing boundary. */
function loader(id: string): ComfyNode {
  return {
    id,
    type: 'SimpleSyrup.LoadUltralyticsModel',
    inputs: [{ name: 'model_name', type: 'COMBO', widget: { name: 'model_name' } }],
  };
}

/** Model Comfy's authoritative boundary resolver, including duplicate fanout links. */
function boundary(id: string, targets: ComfyNode[]): ComfyNode {
  return {
    id,
    type: 'native-subgraph-uuid',
    inputs: [{ name: 'Detector', type: 'COMBO', widget: { name: 'Detector' } }],
    isSubgraphNode: () => true,
    subgraph: {},
    resolveSubgraphInputLinks: (slot: number) =>
      slot === 0 ? targets.map((inputNode) => ({ inputNode, link: { target_slot: 0 } })) : [],
  };
}

test('preserves exact nested instance paths and deduplicates input fanout', () => {
  const leaf = loader('loader');
  const inner = boundary('inner', [leaf, leaf]);
  const outer = boundary('outer', [inner]);
  const subject = new NativeSubgraphBoundaryResolver({ warn: jest.fn() });
  expect(subject.resolveInputTargets(outer, 0)).toEqual([
    { nodeId: 'outer:inner:loader', inputSlot: 0, inputName: 'model_name' },
  ]);
  expect(subject.resolveInputTargets(outer, 1)).toEqual([]);
});

test('resolves renamed promoted widgets to leaf field contracts across nested fanout', () => {
  const outer = boundary('outer', [boundary('inner', [loader('one'), loader('two')])]);
  const subject = new NativeSubgraphBoundaryResolver({ warn: jest.fn() });
  expect(subject.resolveWidgetOrigins(outer, 'Detector')).toEqual([
    { classType: 'SimpleSyrup.LoadUltralyticsModel', inputName: 'model_name' },
  ]);
  expect(subject.resolveWidgetOrigins(outer, 'missing')).toEqual([]);
});

test('terminates circular inputs while preserving independent valid fanout', () => {
  const children: ComfyNode[] = [];
  const outer = boundary('outer', children);
  children.push(outer, loader('leaf'));
  const warn = jest.fn();
  const subject = new NativeSubgraphBoundaryResolver({ warn });
  expect(subject.resolveInputTargets(outer, 0)).toEqual([
    { nodeId: 'outer:leaf', inputSlot: 0, inputName: 'model_name' },
  ]);
  expect(warn).toHaveBeenCalledTimes(1);
});

test('terminates circular output boundaries without changing valid leaf identity', () => {
  const outer = boundary('outer', []);
  outer.resolveSubgraphOutputLink = () => ({ outputNode: outer, link: { origin_slot: 0 } });
  const subject = new NativeSubgraphBoundaryResolver({ warn: jest.fn() });
  expect(subject.resolveOutput(outer, 0)).toBeNull();
  expect(subject.resolveOutput(loader('leaf'), 0)).toEqual({ nodeId: 'leaf', slot: 0 });
});
