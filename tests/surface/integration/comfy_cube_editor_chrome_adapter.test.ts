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
/** Verify native Cube editor chrome and shortcut adaptation. */

import { jest } from '@jest/globals';
import { ComfyCubeEditorChromeAdapter } from '../../../frontend/comfyui/ui/affordance/ComfyCubeEditorChromeAdapter.js';
import { CubeHostAffordanceController } from '../../../frontend/comfyui/ui/affordance/CubeHostAffordanceController.js';
import type { CubeNode } from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { CubeEditorContextResolver } from '../../../frontend/comfyui/ui/surface/CubeEditorContextResolver.js';
import { CubeAffordancePolicy } from '../../../frontend/comfyui/ui/affordance/CubeAffordancePolicy.js';
import type { CubeEditorMetadataHud } from '../../../frontend/comfyui/ui/surface/CubeEditorMetadataHud.js';
import type { NativeGraphNode } from '../../../frontend/comfyui/ui/cube/ComfyCubeGraphBuilder.js';

test('uses Cube language, literal metadata, and Cube save at the editor root', () => {
  document.body.replaceChildren();
  const cube = cubeNode();
  const breadcrumb = element('div', {
    'data-testid': 'subgraph-breadcrumb-item-subgraph-definition',
  });
  const label = element('span', { class: 'p-breadcrumb-item-label' });
  breadcrumb.append(label);
  const back = element('button', {
    'data-testid': 'subgraph-breadcrumb-back',
    'aria-label': 'Exit Subgraph',
  });
  const toggle = element('button', {
    'data-testid': 'subgraph-editor-toggle',
    'aria-label': 'Edit subgraph',
  });
  const menu = element('ul', {
    'data-testid': 'subgraph-breadcrumb-menu-subgraph-definition',
  });
  const rename = menuItem('pi-pencil', 'Rename');
  const clear = menuItem('pi-trash', 'Clear Workflow');
  const search = menuItem('pi-search', 'Set Search Aliases');
  menu.append(rename, clear, search);
  document.body.append(breadcrumb, back, toggle, menu);
  const catalog = new CubeNodeCatalog();
  catalog.add(cube);
  const contexts = new CubeEditorContextResolver(catalog);
  const focus = jest.fn();
  const requestSave = jest.fn(() => true);
  const metadataHud = { focus, requestSave } as unknown as CubeEditorMetadataHud;
  const controller = new CubeHostAffordanceController({
    getRuntime: () => ({ contexts, metadataHud, nodes: catalog }),
    cubeCreation: { saveDraft: jest.fn(async () => null) },
    cubeSave: { save: jest.fn(async () => ({ status: 'saved' })) },
    confirm: { open: jest.fn(async () => false) },
    prepareGraphClear: jest.fn(),
    markGraphDirty: jest.fn(),
    announceGraphCleared: jest.fn(),
  });
  const canvas: { subgraph: object; selectedItems: Set<unknown> } = {
    subgraph: cube.subgraph,
    selectedItems: new Set(),
  };
  const adapter = new ComfyCubeEditorChromeAdapter({
    document,
    canvas,
    contexts,
    policy: new CubeAffordancePolicy(),
    controller,
  });
  adapter.install();

  expect(label.textContent).toBe('<Cube>');
  expect(label.querySelector('b')).toBeNull();
  expect(back.getAttribute('aria-label')).toBe('Exit Cube');
  expect(toggle.hidden).toBe(true);
  expect(toggle.getAttribute('aria-label')).toBe('Edit subgraph');
  expect(rename.textContent).toBe('Edit Cube metadata');
  expect(clear.textContent).toBe('Clear Cube implementation');
  expect(search.hidden).toBe(true);
  const saveEvent = new KeyboardEvent('keydown', {
    key: 's',
    ctrlKey: true,
    bubbles: true,
    cancelable: true,
  });
  document.dispatchEvent(saveEvent);
  expect(saveEvent.defaultPrevented).toBe(true);
  expect(requestSave).toHaveBeenCalledTimes(1);
  breadcrumb.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, cancelable: true }));
  expect(focus).toHaveBeenCalledTimes(1);

  const nestedGraph = { id: 'nested', _nodes: [] };
  cube.subgraph._nodes.push({
    id: 'nested-node',
    pos: [0, 0],
    size: [100, 100],
    properties: {},
    inputs: [],
    outputs: [],
    connect: () => null,
    isSubgraphNode: () => true,
    subgraph: nestedGraph,
  } as NativeGraphNode);
  canvas.subgraph = nestedGraph;
  adapter.refresh();
  expect(label.textContent).toBe('<Cube>');
  expect(back.getAttribute('aria-label')).toBe('Exit Subgraph');
  expect(toggle.getAttribute('aria-label')).toBe('Edit subgraph');
  expect(toggle.hidden).toBe(false);
  expect(rename.textContent).toBe('Rename');
  expect(clear.textContent).toBe('Clear Workflow');
  expect(search.hidden).toBe(false);
  adapter.dispose();
});

/** Create one PrimeVue-style icon menu item. */
function menuItem(iconClass: string, label: string): HTMLLIElement {
  const item = document.createElement('li');
  item.append(element('i', { class: iconClass }), element('span', { class: 'p-menu-item-label' }));
  item.querySelector<HTMLElement>('.p-menu-item-label')!.textContent = label;
  return item;
}

/** Create one typed DOM element with explicit attributes. */
function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attributes: Record<string, string>,
): HTMLElementTagNameMap[K] {
  const value = document.createElement(tag);
  for (const [name, content] of Object.entries(attributes)) value.setAttribute(name, content);
  return value;
}

/** Build a Cube whose title verifies literal DOM rendering. */
function cubeNode(): CubeNode {
  return {
    id: 'cube-1',
    type: 'definition',
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'cube-1', cube_id: 'cube.cube', default_alias: '<Cube>' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph: {
      id: 'definition',
      name: 'Native Subgraph name',
      _nodes: [],
      inputs: [],
      outputs: [],
    } as unknown as CubeNode['subgraph'],
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}
