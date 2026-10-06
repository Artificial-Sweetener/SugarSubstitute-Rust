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
/** Verify graph and masonry views of the same face retain independent mount lifetimes. */
import { afterEach, expect, test } from '@jest/globals';
import { NativeRelationViews } from '../../../frontend/comfyui/ui/relations/infrastructure/NativeRelationViews.js';
import type { ComfyNode } from '../../../frontend/comfyui/ui/types/graph.js';

afterEach(() => document.body.replaceChildren());

test('discovers a second mounted view while the first remains connected and releases removed views', () => {
  const node: ComfyNode = { id: 1, graph: { id: 'cube-graph' } };
  const registry = new NativeRelationViews(document);
  const graph = document.createElement('div');
  graph.className = 'sugarcubes-cube-face__node-card';
  graph.dataset.cubeNodeLocator = 'cube-graph:1';
  document.body.append(graph);
  expect(registry.read(node, 'cube')).toHaveLength(1);
  const other: ComfyNode = { id: 1, graph: { id: 'other-cube-graph' } };
  const otherRoot = document.createElement('div');
  otherRoot.className = 'sugarcubes-cube-face__node-card';
  otherRoot.dataset.cubeNodeLocator = 'other-cube-graph:1';
  document.body.append(otherRoot);
  const retainedOther = registry.read(other, 'other-cube');
  const masonry = graph.cloneNode(false);
  if (!(masonry instanceof HTMLElement)) throw new Error('Expected native cell.');
  document.body.append(masonry);
  registry.invalidate([masonry]);
  expect(registry.read(other, 'other-cube')).toBe(retainedOther);
  expect(registry.read(node, 'cube').map((view) => view.root)).toEqual([graph, masonry]);
  graph.remove();
  registry.invalidate([graph]);
  expect(registry.read(node, 'cube').map((view) => view.root)).toEqual([masonry]);
  registry.clear();
});
