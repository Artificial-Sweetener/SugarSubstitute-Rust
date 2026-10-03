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
/** Safeguard masonry-only icon adoption across native Vue header replacement. */
import { afterEach, expect, test } from '@jest/globals';
import { MasonryNodeIconHost } from '../../../frontend/comfyui/ui/surface/masonry/MasonryNodeIconHost.js';

afterEach(() => document.body.replaceChildren());

/** Build a native header without replacing custom content or interaction handlers. */
function nativeHeader() {
  const title = document.createElement('div');
  title.dataset.testid = 'node-title';
  const text = document.createElement('span');
  text.textContent = '<custom title>';
  title.append(text);
  const header = document.createElement('header');
  header.dataset.testid = 'node-header-1';
  header.append(title);
  return { title, header, text };
}

test('adopts one decorative icon into a replaceable header, refreshes semantics, and releases it', async () => {
  const cell = document.createElement('article');
  cell.dataset.masonryCard = '1';
  const first = nativeHeader();
  cell.append(first.header);
  document.body.append(cell);
  const node = { id: 1, type: 'KSampler' };
  const host = new MasonryNodeIconHost(cell, node);
  expect(first.title.querySelector('[data-masonry-node-icon]')?.getAttribute('aria-hidden')).toBe(
    'true',
  );
  expect(first.title.dataset.masonryTitleIcon).toBe('');
  expect(first.title.textContent).toBe('<custom title>');
  const second = nativeHeader();
  cell.replaceChildren(second.header);
  await Promise.resolve();
  expect(
    second.title.querySelector('[data-masonry-node-icon]')?.getAttribute('data-masonry-node-icon'),
  ).toBe('application');
  expect(first.title.hasAttribute('data-masonry-title-icon')).toBe(false);
  node.type = 'VectorscopeCC';
  host.refresh();
  expect(
    second.title.querySelector('[data-masonry-node-icon]')?.getAttribute('data-masonry-node-icon'),
  ).toBe('palette');
  node.type = 'Unknown';
  host.refresh();
  expect(second.title.querySelector('[data-masonry-node-icon]')).toBeNull();
  expect(second.title.hasAttribute('data-masonry-title-icon')).toBe(false);
  host.dispose();
  node.type = 'KSampler';
  cell.replaceChildren(first.header);
  await Promise.resolve();
  expect(first.title.querySelector('[data-masonry-node-icon]')).toBeNull();
});

test('does not adopt into a graph header or a nested descendant card', () => {
  const graph = document.createElement('article');
  const native = nativeHeader();
  graph.append(native.header);
  const host = new MasonryNodeIconHost(graph, { id: 1, type: 'KSampler' });
  expect(graph.querySelector('[data-masonry-node-icon]')).toBeNull();
  host.dispose();
  graph.dataset.masonryCard = 'outer';
  const nested = document.createElement('article');
  nested.dataset.masonryCard = 'inner';
  nested.append(native.header);
  graph.append(nested);
  const outer = new MasonryNodeIconHost(graph, { id: 1, type: 'KSampler' });
  expect(nested.querySelector('[data-masonry-node-icon]')).toBeNull();
  outer.dispose();
});
