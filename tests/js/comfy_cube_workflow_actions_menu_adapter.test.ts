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
/** Verify Cube language on Comfy's portalled Workflow actions menu. */

import { CubeAffordancePolicy } from '../../frontend/comfyui/ui/affordance/CubeAffordancePolicy.js';
import { ComfyCubeWorkflowActionsMenuAdapter } from '../../frontend/comfyui/ui/affordance/ComfyCubeWorkflowActionsMenuAdapter.js';
import type { CubeNode } from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { CubeNodeCatalog } from '../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { CubeEditorContextResolver } from '../../frontend/comfyui/ui/surface/CubeEditorContextResolver.js';

test('relabels only primary save and clear at the Cube root and restores nested context', () => {
  document.body.replaceChildren();
  const cube = cubeNode();
  const menu = document.createElement('div');
  menu.setAttribute('role', 'menu');
  menu.setAttribute('data-reka-menu-content', '');
  menu.setAttribute('data-state', 'open');
  const save = workflowMenuItem('pi-save', 'Save');
  const saveAs = workflowMenuItem('pi-save', 'Save As');
  const exportItem = workflowMenuItem('pi-download', 'Export');
  const clear = workflowMenuItem('pi-trash', 'Clear Workflow');
  const deleteItem = workflowMenuItem('pi-times', 'Delete Workflow');
  menu.append(save, saveAs, exportItem, clear, deleteItem);
  document.body.append(menu);
  const catalog = new CubeNodeCatalog();
  catalog.add(cube);
  const contexts = new CubeEditorContextResolver(catalog);
  const canvas: { subgraph: object; selectedItems: Set<unknown> } = {
    subgraph: cube.subgraph,
    selectedItems: new Set(),
  };
  const adapter = new ComfyCubeWorkflowActionsMenuAdapter({
    document,
    canvas,
    contexts,
    policy: new CubeAffordancePolicy(),
  });
  adapter.install();

  expect(save.textContent).toBe('Save Cube');
  expect(saveAs.textContent).toBe('Save As');
  expect(clear.textContent).toBe('Clear Cube implementation');
  const nested = { id: 'nested', _nodes: [] };
  cube.subgraph._nodes.push({
    isSubgraphNode: () => true,
    subgraph: nested,
  } as unknown as CubeNode);
  canvas.subgraph = nested;
  adapter.refresh();
  expect(save.textContent).toBe('Save');
  expect(clear.textContent).toBe('Clear Workflow');
  adapter.dispose();
});

/** Create one Reka workflow action with semantic host markers. */
function workflowMenuItem(iconClass: string, label: string): HTMLDivElement {
  const item = document.createElement('div');
  item.setAttribute('role', 'menuitem');
  const icon = document.createElement('i');
  icon.className = iconClass;
  const text = document.createElement('span');
  text.className = 'flex-1';
  text.textContent = label;
  item.append(icon, text);
  return item;
}

/** Build one saved Cube definition for editor context resolution. */
function cubeNode(): CubeNode {
  return {
    id: 'cube-1',
    type: 'definition',
    title: 'Cube',
    pos: [0, 0],
    size: [720, 480],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: { instance_id: 'cube-1', cube_id: 'cube.cube' },
      sugarcubes_surface: {},
    },
    inputs: [],
    outputs: [],
    subgraph: {
      id: 'definition',
      name: 'Cube: Cube',
      _nodes: [],
      inputs: [],
      outputs: [],
    } as unknown as CubeNode['subgraph'],
    isSubgraphNode: () => true,
    connect() {},
    serialize: () => ({}),
  };
}
