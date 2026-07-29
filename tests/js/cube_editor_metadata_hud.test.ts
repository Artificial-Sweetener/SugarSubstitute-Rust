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
/** Verify the viewport-pinned Cube metadata HUD behavior. */

import { jest } from '@jest/globals';
import { CubeEditorMetadataHud } from '../../frontend/comfyui/ui/surface/CubeEditorMetadataHud.js';
import {
  requireCubeIdentity,
  type CubeNode,
} from '../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';

test('retains a dirty Save control after the editable metadata card is rolled up', async () => {
  document.body.replaceChildren();
  const save = jest.fn(async () => 'saved' as const);
  const hud = new CubeEditorMetadataHud(document, {
    canEdit: async () => true,
    save,
  });
  const node = cubeNode();

  hud.show(node);
  await Promise.resolve();

  const name = document.querySelector<HTMLInputElement>('.sugarcubes-cube-editor-metadata input');
  if (!name) throw new Error('Expected editable Cube name input.');
  name.value = 'Polished Cube';
  name.dispatchEvent(new Event('input'));
  document.querySelector<HTMLButtonElement>('.sugarcubes-cube-editor-metadata__collapse')?.click();

  expect(document.querySelector('.sugarcubes-cube-editor-metadata')?.textContent).toContain(
    'Polished Cube',
  );
  expect(document.querySelector('.sugarcubes-cube-unsaved-indicator')).toBeNull();
  const saveButton = document.querySelector<HTMLButtonElement>(
    '.sugarcubes-cube-editor-metadata__save',
  );
  expect(saveButton?.disabled).toBe(false);
  saveButton?.click();
  await Promise.resolve();

  expect(save).toHaveBeenCalledWith(
    node,
    expect.objectContaining({ defaultAlias: 'Polished Cube' }),
  );
  hud.dispose();
});

test('retains text focus and selection across metadata card refreshes', async () => {
  document.body.replaceChildren();
  const hud = new CubeEditorMetadataHud(document, {
    canEdit: async () => true,
    save: async () => 'saved',
  });

  hud.show(cubeNode());
  await Promise.resolve();
  const initialName = document.querySelector<HTMLInputElement>(
    '.sugarcubes-cube-editor-metadata input',
  );
  if (!initialName) throw new Error('Expected editable Cube name input.');
  initialName.focus();
  initialName.value = 'A';
  initialName.setSelectionRange(1, 1);
  initialName.dispatchEvent(new Event('input'));

  const refreshedName = document.querySelector<HTMLInputElement>(
    '.sugarcubes-cube-editor-metadata input',
  );
  expect(document.activeElement).toBe(refreshedName);
  expect(refreshedName?.selectionStart).toBe(1);
  if (!refreshedName) throw new Error('Expected refreshed Cube name input.');
  refreshedName.value = 'AB';
  refreshedName.setSelectionRange(2, 2);
  refreshedName.dispatchEvent(new Event('input'));

  expect(document.activeElement).toBe(
    document.querySelector('.sugarcubes-cube-editor-metadata input'),
  );
  expect(
    document.querySelector<HTMLInputElement>('.sugarcubes-cube-editor-metadata input')?.value,
  ).toBe('AB');
  hud.dispose();
});

test('uses Comfy PrimeIcons and clears the native toolbar, sidebar, and breadcrumb', async () => {
  document.body.replaceChildren();
  const toolbar = document.createElement('nav');
  toolbar.className = 'side-tool-bar-container';
  Object.defineProperty(toolbar, 'getBoundingClientRect', {
    value: () => ({ left: 0, right: 74, top: 38, bottom: 900, width: 74, height: 862 }),
  });
  const sidebar = document.createElement('div');
  sidebar.className = 'side-bar-panel';
  Object.defineProperty(sidebar, 'getBoundingClientRect', {
    value: () => ({ left: 0, right: 280, top: 38, bottom: 900, width: 280, height: 862 }),
  });
  const breadcrumb = document.createElement('div');
  breadcrumb.className = 'subgraph-breadcrumb';
  Object.defineProperty(breadcrumb, 'getBoundingClientRect', {
    value: () => ({ left: 80, right: 400, top: 64, bottom: 102, width: 320, height: 38 }),
  });
  document.body.append(toolbar, sidebar, breadcrumb);
  const hud = new CubeEditorMetadataHud(document, {
    canEdit: async () => true,
    save: async () => 'saved',
  });

  hud.show(cubeNode());
  await Promise.resolve();

  const card = document.querySelector<HTMLElement>('.sugarcubes-cube-editor-metadata');
  expect(card?.style.getPropertyValue('--sugarcubes-cube-editor-metadata-left')).toBe('296px');
  expect(card?.style.getPropertyValue('--sugarcubes-cube-editor-metadata-top')).toBe('118px');
  expect(document.querySelector('.sugarcubes-cube-editor-metadata__save .pi-save')).not.toBeNull();
  expect(
    document.querySelector('.sugarcubes-cube-editor-metadata__collapse .pi-chevron-up'),
  ).not.toBeNull();
  hud.dispose();
});

test('repositions when Comfy expands a left panel after the HUD mounts', async () => {
  document.body.replaceChildren();
  let panelRight = 0;
  const toolbar = document.createElement('nav');
  toolbar.className = 'side-tool-bar-container';
  Object.defineProperty(toolbar, 'getBoundingClientRect', {
    value: () => ({ left: 0, right: 74, top: 38, bottom: 900, width: 74, height: 862 }),
  });
  const panel = document.createElement('div');
  panel.className = 'side-bar-panel';
  Object.defineProperty(panel, 'getBoundingClientRect', {
    value: () => ({
      left: 74,
      right: panelRight ? panelRight : 74,
      top: 38,
      bottom: 900,
      width: panelRight ? panelRight - 74 : 0,
      height: panelRight ? 862 : 0,
    }),
  });
  document.body.append(toolbar, panel);
  const hud = new CubeEditorMetadataHud(document, {
    canEdit: async () => true,
    save: async () => 'saved',
  });

  hud.show(cubeNode());
  await Promise.resolve();
  const card = document.querySelector<HTMLElement>('.sugarcubes-cube-editor-metadata');
  expect(card?.style.getPropertyValue('--sugarcubes-cube-editor-metadata-left')).toBe('90px');

  panelRight = 320;
  panel.classList.add('is-open');
  await Promise.resolve();
  await Promise.resolve();

  expect(card?.style.getPropertyValue('--sugarcubes-cube-editor-metadata-left')).toBe('336px');
  hud.dispose();
});

test('mounts above Comfy workspace chrome so metadata actions cannot click through', async () => {
  document.body.replaceChildren();
  const appRoot = document.createElement('div');
  appRoot.id = 'vue-app';
  document.body.append(appRoot);
  const hud = new CubeEditorMetadataHud(document, {
    canEdit: async () => true,
    save: async () => 'saved',
  });

  hud.show(cubeNode());
  await Promise.resolve();

  const card = document.querySelector<HTMLElement>('.sugarcubes-cube-editor-metadata');
  expect(card?.parentElement).toBe(document.body);
  hud.dispose();
});

test('renders non-owned Cube metadata as read only', async () => {
  document.body.replaceChildren();
  const hud = new CubeEditorMetadataHud(document, {
    canEdit: async () => false,
    save: async () => 'saved',
  });

  hud.show(cubeNode());
  await Promise.resolve();

  expect(document.querySelectorAll('.sugarcubes-cube-editor-metadata input')).toHaveLength(0);
  expect(document.querySelector('.sugarcubes-cube-editor-metadata')?.textContent).toContain(
    'Read only',
  );
  expect(document.querySelector('.sugarcubes-cube-editor-metadata__save')).toBeNull();
  hud.dispose();
});

test('keeps a failed save actionable and explains why it failed', async () => {
  document.body.replaceChildren();
  const hud = new CubeEditorMetadataHud(document, {
    canEdit: async () => true,
    save: async () => Promise.reject(new Error('Cube export failed.')),
  });
  const node = cubeNode();

  hud.show(node);
  await Promise.resolve();
  const name = document.querySelector<HTMLInputElement>('.sugarcubes-cube-editor-metadata input');
  if (!name) throw new Error('Expected editable Cube name input.');
  name.value = 'Retry Cube';
  name.dispatchEvent(new Event('input'));
  document.querySelector<HTMLButtonElement>('.sugarcubes-cube-editor-metadata__save')?.click();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();

  expect(document.querySelector('[role="alert"]')?.textContent).toContain('Cube export failed.');
  expect(
    document.querySelector<HTMLButtonElement>('.sugarcubes-cube-editor-metadata__save')?.disabled,
  ).toBe(false);
  hud.dispose();
});

test('keeps a dismissed first-save dialog actionable without reporting an error', async () => {
  document.body.replaceChildren();
  const hud = new CubeEditorMetadataHud(document, {
    canEdit: async () => true,
    save: async () => 'cancelled',
  });
  const node = cubeNode();

  hud.show(node);
  await Promise.resolve();
  const name = document.querySelector<HTMLInputElement>('.sugarcubes-cube-editor-metadata input');
  if (!name) throw new Error('Expected editable Cube name input.');
  name.value = 'Still Editing Cube';
  name.dispatchEvent(new Event('input'));
  document.querySelector<HTMLButtonElement>('.sugarcubes-cube-editor-metadata__save')?.click();
  await Promise.resolve();
  await Promise.resolve();

  expect(document.querySelector('[role="alert"]')).toBeNull();
  expect(
    document.querySelector<HTMLButtonElement>('.sugarcubes-cube-editor-metadata__save')?.disabled,
  ).toBe(false);
  expect(document.querySelector('.sugarcubes-cube-editor-metadata')?.textContent).toContain(
    'Still Editing Cube',
  );
  hud.dispose();
});

test('uses the established first-save model defaults for a blank Cube draft', async () => {
  document.body.replaceChildren();
  const hud = new CubeEditorMetadataHud(document, {
    canEdit: async () => true,
    save: async () => 'saved',
  });
  const node = cubeNode();
  const identity = requireCubeIdentity(node);
  node.properties.sugarcubes_kind = 'cube_draft';
  delete identity.cube_id;
  delete identity.target_model;
  delete identity.supported_models;
  node.subgraph = {
    id: 'draft-subgraph',
    name: 'Cube: Untitled Cube',
    _nodes: [],
    inputs: [],
    outputs: [],
  } as never;

  hud.show(node);
  await Promise.resolve();

  const targetModel = document.querySelector<HTMLSelectElement>(
    '.sugarcubes-cube-editor-metadata__target-model select',
  );
  const supportedModels = document.querySelector<HTMLInputElement>(
    '.sugarcubes-cube-editor-metadata__model-support input',
  );
  expect(targetModel?.value).toBe('SDXL');
  expect(supportedModels?.value).toBe('SDXL');
  const indicator = document.querySelector<HTMLElement>('.sugarcubes-cube-unsaved-indicator');
  expect(indicator?.getAttribute('aria-label')).toBe('Not saved yet');
  expect(indicator?.querySelector('.pi-save')).not.toBeNull();
  expect(indicator?.querySelector('.pi-ban')).not.toBeNull();
  expect(
    document.querySelector<HTMLButtonElement>('.sugarcubes-cube-editor-metadata__save')?.disabled,
  ).toBe(false);
  hud.dispose();
});

/** Build the narrow graph-owned Cube metadata shape required by the HUD. */
function cubeNode(): CubeNode {
  return {
    id: 'cube-instance',
    title: 'Untitled Cube',
    pos: [0, 0],
    size: [320, 180],
    properties: {
      sugarcubes_kind: 'cube',
      sugarcubes_cube: {
        instance_id: 'cube-instance',
        cube_id: 'local/personal/SDXL/untitled.cube',
        default_alias: 'Untitled Cube',
        target_model: 'SDXL',
        supported_models: ['SDXL'],
        description: 'A draft-ready cube.',
      },
    },
    inputs: [],
    outputs: [],
    subgraph: {} as never,
    connect() {},
    isSubgraphNode: () => true,
    serialize: () => ({}),
  } as CubeNode;
}
