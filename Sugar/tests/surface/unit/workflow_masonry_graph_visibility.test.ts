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

test('holds input out of the layers without inert while borrowed cards return', () => {
  const root = document.createElement('div');
  root.id = 'graph-canvas-container';
  const container = document.createElement('div');
  const canvas = document.createElement('canvas');
  canvas.id = 'graph-canvas';
  canvas.inert = true;
  const nodes = document.createElement('div');
  nodes.className = 'isolate';
  nodes.inert = false;
  const button = document.createElement('button');
  nodes.append(button);
  root.append(canvas, nodes, container);
  document.body.append(root);
  const visibility = new WorkflowMasonryGraphVisibility(() => container);
  visibility.setInert(true);
  visibility.holdInput();
  // A layer that was inert before the masonry stays inert; the other is held instead.
  expect([canvas.inert, nodes.inert]).toEqual([true, false]);
  expect(nodes.hasAttribute('data-sc-input-held')).toBe(true);
  let clicks = 0;
  button.addEventListener('click', () => (clicks += 1));
  button.click();
  expect(clicks).toBe(0);
  visibility.setInert(false);
  expect([canvas.inert, nodes.inert]).toEqual([true, false]);
  expect(nodes.hasAttribute('data-sc-input-held')).toBe(false);
  button.click();
  expect(clicks).toBe(1);
  // Entering again while held goes back to inert.
  visibility.setInert(true);
  visibility.holdInput();
  visibility.setInert(true);
  expect(nodes.inert).toBe(true);
  expect(nodes.hasAttribute('data-sc-input-held')).toBe(false);
});

test('hides everything in the graph container that is not SugarCubes’ own, whenever it mounts', async () => {
  const root = document.createElement('div');
  root.id = 'graph-canvas-container';
  const container = document.createElement('div');
  container.className = 'sc-output-workspace-frame';
  const canvas = document.createElement('canvas');
  canvas.id = 'graph-canvas';
  // What the host and other extensions mount beside the graph: none of it is on any list.
  const previews = document.createElement('canvas');
  previews.className = 'pointer-events-none absolute inset-0';
  const toolbar = document.createElement('div');
  toolbar.className = 'pointer-events-none fixed top-0 left-0';
  const extension = document.createElement('div');
  extension.id = 'some-extension-root';
  const adopted = document.createElement('div');
  adopted.setAttribute('data-sc-desktop-frame-surface', '');
  root.append(canvas, previews, toolbar, extension, adopted, container);
  document.body.append(root);
  const visibility = new WorkflowMasonryGraphVisibility(() => container);
  const hidden = (element: Element): boolean =>
    element.getAttribute('data-sc-masonry-hides') === 'graph';

  visibility.setHidden(true);

  expect([canvas, previews, toolbar, extension].map(hidden)).toEqual([true, true, true, true]);
  expect([adopted, container].map(hidden)).toEqual([false, false]);
  // Something mounted while masonry is shown is hidden as it arrives.
  const late = document.createElement('div');
  late.className = 'node-tooltip';
  const ours = document.createElement('div');
  ours.className = 'sugarcubes-workflow-masonry__backdrop';
  root.append(late, ours);
  await new Promise((resolve) => setTimeout(resolve, 0));
  await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
  expect(hidden(late)).toBe(true);
  expect(hidden(ours)).toBe(false);

  visibility.setHidden(false);
  expect(root.querySelector('[data-sc-masonry-hides]')).toBeNull();
  const after = document.createElement('div');
  root.append(after);
  await new Promise((resolve) => setTimeout(resolve, 0));
  await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
  expect(hidden(after)).toBe(false);
});
