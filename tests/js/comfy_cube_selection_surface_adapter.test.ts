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
/** Verify Vue toolbox and More Options adaptation through stable host anchors. */

import { ComfyCubeSelectionSurfaceAdapter } from '../../frontend/comfyui/ui/affordance/ComfyCubeSelectionSurfaceAdapter.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { CubeEditorContextResolver } from '../../frontend/comfyui/ui/surface/CubeEditorContextResolver.js';

test('hides structural actions and relabels native Cube actions without parsing labels', () => {
  document.body.replaceChildren();
  const toolbox = element('div', { 'data-testid': 'selection-toolbox' });
  const unpack = buttonWithIcon('icon-[lucide--expand]');
  unpack.setAttribute('data-testid', 'convert-to-subgraph-button');
  const publish = buttonWithIcon('icon-[lucide--book-open]');
  publish.setAttribute('aria-label', 'Publish Subgraph');
  publish.setAttribute('aria-describedby', 'publish-tooltip');
  const configure = buttonWithIcon('icon-[lucide--settings-2]');
  toolbox.append(unpack, publish, configure);
  const menu = element('div', { class: 'p-contextmenu' });
  const libraryItem = element('li');
  libraryItem.append(element('i', { class: 'icon-[lucide--folder-plus]' }));
  const infoItem = element('li');
  infoItem.append(element('i', { class: 'pi pi-info-circle' }));
  menu.append(libraryItem, infoItem);
  const tooltip = element('div', { id: 'publish-tooltip' });
  tooltip.append(element('span', { class: 'p-tooltip-text' }));
  tooltip.querySelector<HTMLElement>('.p-tooltip-text')!.textContent = 'Publish Subgraph';
  document.body.append(toolbox, menu, tooltip);
  const canvas = { selectedItems: new Set<unknown>([cubeNode()]) };
  const adapter = new ComfyCubeSelectionSurfaceAdapter({
    document,
    canvas,
    contexts: new CubeEditorContextResolver(new CubeNodeCatalog()),
  });

  adapter.refresh();
  expect(unpack.hidden).toBe(true);
  expect(libraryItem.hidden).toBe(true);
  expect(libraryItem.style.getPropertyPriority('display')).toBe('important');
  expect(infoItem.hidden).toBe(true);
  expect(publish.getAttribute('aria-label')).toBe('Save Cube');
  expect(tooltip.textContent).toBe('Save Cube');
  expect(configure.hidden).toBe(true);
  canvas.selectedItems = new Set();
  adapter.refresh();
  expect(unpack.hidden).toBe(false);
  expect(libraryItem.hidden).toBe(false);
  expect(libraryItem.style.display).toBe('');
  expect(publish.getAttribute('aria-label')).toBe('Publish Subgraph');
  expect(tooltip.textContent).toBe('Publish Subgraph');
  expect(configure.hidden).toBe(false);
});

/** Create one element with literal attributes. */
function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attributes: Record<string, string> = {},
): HTMLElementTagNameMap[K] {
  const value = document.createElement(tag);
  for (const [name, content] of Object.entries(attributes)) value.setAttribute(name, content);
  return value;
}

/** Create one button around a semantic Comfy icon class. */
function buttonWithIcon(iconClass: string): HTMLButtonElement {
  const button = document.createElement('button');
  button.append(element('i', { class: iconClass }));
  return button;
}

/** Build one complete marked Cube selection operand. */
function cubeNode(): CubeNode {
  return {
    id: 'cube-1',
    type: 'definition',
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'cube-1' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph: {
      id: 'definition',
      name: 'Cube',
      _nodes: [],
      inputs: [],
      outputs: [],
    } as unknown as CubeNode['subgraph'],
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}
