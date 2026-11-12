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
/** Prove graph suppression outlives card mounts without retaining native interaction changes. */
import { afterEach, expect, test } from '@jest/globals';
import { WorkflowMasonryGraphVisibility } from '../../../frontend/comfyui/ui/surface/masonry/WorkflowMasonryGraphVisibility.js';

afterEach(() => document.body.replaceChildren());

test('suppresses a workspace before masonry mounts and restores native inert state', () => {
  const root = document.createElement('div');
  root.id = 'graph-canvas-container';
  const container = document.createElement('div');
  const canvas = document.createElement('canvas');
  canvas.id = 'graph-canvas';
  canvas.inert = true;
  const nodes = document.createElement('div');
  nodes.className = 'isolate';
  nodes.inert = false;
  root.append(canvas, nodes, container);
  document.body.append(root);
  const visibility = new WorkflowMasonryGraphVisibility(() => container);
  const changes = new MutationObserver(() => undefined);
  changes.observe(root, { attributes: true });
  visibility.setHidden(true);
  expect(changes.takeRecords()).toHaveLength(1);
  visibility.setHidden(true);
  expect(changes.takeRecords()).toHaveLength(0);
  changes.disconnect();
  expect(root.dataset.sugarcubesMasonry).toBe('true');
  expect(container.dataset.sugarcubesMasonry).toBe('true');
  expect(nodes.inert).toBe(true);
  visibility.setHidden(false);
  expect(root.dataset.sugarcubesMasonry).toBeUndefined();
  expect(container.dataset.sugarcubesMasonry).toBeUndefined();
  expect(canvas.inert).toBe(true);
  expect(nodes.inert).toBe(false);
});

test('releases suppression on every retained allocation after host replacement', () => {
  const first = document.createElement('div');
  const second = document.createElement('div');
  let allocation: HTMLElement | null = null;
  const visibility = new WorkflowMasonryGraphVisibility(() => allocation);
  visibility.setHidden(true);
  allocation = first;
  visibility.setHidden(true);
  allocation = second;
  visibility.setHidden(true);
  allocation = null;
  visibility.setHidden(false);
  expect(first.dataset.sugarcubesMasonry).toBeUndefined();
  expect(second.dataset.sugarcubesMasonry).toBeUndefined();
});
