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
/** Verify the shared Comfy root mutation boundary preserves host ownership. */

import { jest } from '@jest/globals';
import { ComfyCubeRootMutationAdapter } from '../../../frontend/comfyui/ui/cube/ComfyCubeRootMutationAdapter.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { NativeSubgraphChangeObserver } from '../../../frontend/comfyui/ui/surface/NativeSubgraphChangeObserver.js';

test('binds root mutations and narrows dynamic link values', () => {
  const node = { id: 'cube' } as CubeNode;
  const add = jest.fn();
  const remove = jest.fn();
  const getNodeById = jest.fn(() => node);
  const getLink = jest.fn((id: unknown) => (id === 1 ? { id: 1 } : 'invalid'));
  const adapter = new ComfyCubeRootMutationAdapter({ add, remove, getNodeById, getLink });

  adapter.add(node);
  adapter.remove(node);

  expect(add).toHaveBeenCalledWith(node);
  expect(remove).toHaveBeenCalledWith(node);
  expect(adapter.getNodeById('cube')).toBe(node);
  expect(adapter.getLink(1)).toEqual({ id: 1 });
  expect(adapter.getLink(2)).toBeNull();
});

test('notifies observers installed after adapter construction and after observer replacement', async () => {
  const node = { id: 'cube' } as CubeNode;
  const graph = { add: jest.fn(), remove: jest.fn(), getNodeById: jest.fn(), getLink: jest.fn() };
  const adapter = new ComfyCubeRootMutationAdapter(graph);
  const relations = jest.fn();
  const outputs = jest.fn();
  const relationObserver = new NativeSubgraphChangeObserver(graph, relations);
  let outputObserver = new NativeSubgraphChangeObserver(graph, outputs);
  adapter.add(node);
  await Promise.resolve();
  expect(relations).toHaveBeenCalledWith([{ kind: 'add', info: node }]);
  expect(outputs).toHaveBeenCalledWith([{ kind: 'add', info: node }]);
  outputObserver.dispose();
  outputObserver = new NativeSubgraphChangeObserver(graph, outputs);
  adapter.remove(node);
  await Promise.resolve();
  expect(relations).toHaveBeenLastCalledWith([{ kind: 'remove', info: node }]);
  expect(outputs).toHaveBeenLastCalledWith([{ kind: 'remove', info: node }]);
  outputObserver.dispose();
  relationObserver.dispose();
});

test('fails closed if a required host method becomes unavailable', () => {
  const graph = { add: jest.fn(), remove: jest.fn(), getNodeById: jest.fn(), getLink: jest.fn() };
  const adapter = new ComfyCubeRootMutationAdapter(graph);
  Reflect.set(graph, 'add', null);
  expect(() => adapter.add({ id: 'cube' } as CubeNode)).toThrow('LGraph.add is unavailable.');
});
