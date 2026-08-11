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
import { CubeEditorMetadataHud } from '../../../frontend/comfyui/ui/surface/CubeEditorMetadataHud.js';
import {
  requireCubeIdentity,
  type CubeNode,
} from '../../../frontend/comfyui/ui/cube/node/ComfyCubeNodeFactory.js';
import { TestComfySettingsSelectRenderer } from '../../settings/support/ComfySettingsSelectTestRenderer.js';
import {
  CUBE_EDITOR_WORKSPACE_CLASS,
  CubeEditorWorkspaceChromeAdapter,
} from '../../../frontend/comfyui/ui/surface/CubeEditorWorkspaceChromeAdapter.js';

test('marks only the active Cube editor workspace for sidebar suppression', async () => {
  document.body.replaceChildren();
  document.body.className = '';
  const hud = new CubeEditorMetadataHud(
    document,
    {
      canEdit: async () => true,
      save: async () => 'saved',
    },
    { workspaceChrome: new CubeEditorWorkspaceChromeAdapter(document) },
  );

  hud.show(cubeNode());
  await Promise.resolve();
  expect(document.body.classList.contains(CUBE_EDITOR_WORKSPACE_CLASS)).toBe(true);

  hud.hide();
  expect(document.body.classList.contains(CUBE_EDITOR_WORKSPACE_CLASS)).toBe(false);
  hud.dispose();
});

test('rolls up persisted Cube metadata while keeping first-save metadata available', async () => {
  document.body.replaceChildren();
  const hud = new CubeEditorMetadataHud(document, {
    canEdit: async () => true,
    save: async () => 'saved',
  });
  const persisted = cubeNode();

  hud.show(persisted);
  await Promise.resolve();

  expect(document.querySelector('[aria-label="Expand"]')).not.toBeNull();
  expect(document.querySelector('.sugarcubes-cube-editor-metadata__body')).toBeNull();

  hud.hide();
  const draft = cubeNode();
  draft.properties.sugarcubes_kind = 'cube_draft';
  delete requireCubeIdentity(draft).cube_id;
  hud.show(draft);
  await Promise.resolve();

  expect(document.querySelector('[aria-label="Minimize"]')).not.toBeNull();
  expect(document.querySelector('.sugarcubes-cube-editor-metadata__body')).not.toBeNull();
  hud.dispose();
});

test('retains a dirty Save control after the editable metadata card is rolled up', async () => {
  document.body.replaceChildren();
  const save = jest.fn(async () => 'saved' as const);
  const hud = new CubeEditorMetadataHud(document, {
    canEdit: async () => true,
    save,
  });
  const node = cubeNode();

  await showExpanded(hud, node);

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

  await showExpanded(hud, cubeNode());
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

test('retains unsaved metadata while navigating away from and back into a cube graph', async () => {
  document.body.replaceChildren();
  const settingsSelect = new TestComfySettingsSelectRenderer();
  const hud = new CubeEditorMetadataHud(
    document,
    {
      canEdit: async () => true,
      save: async () => 'saved',
    },
    { settingsSelectRenderer: settingsSelect },
  );
  const node = cubeNode();

  await showExpanded(hud, node);
  const name = document.querySelector<HTMLInputElement>(
    '[data-sugarcubes-focus-key="defaultAlias"]',
  );
  const description = document.querySelector<HTMLTextAreaElement>(
    '[data-sugarcubes-focus-key="description"]',
  );
  if (!name || !description) throw new Error('Expected editable Cube metadata fields.');
  name.value = 'Unsaved Navigation Draft';
  name.dispatchEvent(new Event('input'));
  description.value = 'Keep this description while navigating.';
  description.dispatchEvent(new Event('input'));
  settingsSelect.enterAutocomplete('Supported models', ['SDXL', 'Custom Runtime']);

  hud.hide();
  expect(document.querySelector('.sugarcubes-cube-editor-metadata')).toBeNull();
  await showExpanded(hud, node);

  expect(
    document.querySelector<HTMLInputElement>('[data-sugarcubes-focus-key="defaultAlias"]')?.value,
  ).toBe('Unsaved Navigation Draft');
  expect(
    document.querySelector<HTMLTextAreaElement>('[data-sugarcubes-focus-key="description"]')?.value,
  ).toBe('Keep this description while navigating.');
  expect(settingsSelect.autocomplete('Supported models').values).toEqual([
    'SDXL',
    'Custom Runtime',
  ]);
  expect(
    document.querySelector<HTMLButtonElement>('.sugarcubes-cube-editor-metadata__save')?.disabled,
  ).toBe(false);
  hud.dispose();
});

test('uses Comfy Settings AutoComplete for suggested and custom model support', async () => {
  document.body.replaceChildren();
  const settingsSelect = new TestComfySettingsSelectRenderer();
  const hud = new CubeEditorMetadataHud(
    document,
    {
      canEdit: async () => true,
      save: async () => 'saved',
      modelSuggestions: () => ['SDXL', 'Flux .1 D', 'Flux .1 Kontext', 'SD 1.5'],
    },
    { settingsSelectRenderer: settingsSelect },
  );

  await showExpanded(hud, cubeNode());

  settingsSelect.completeAutocomplete('Supported models', 'flux');
  expect(settingsSelect.autocomplete('Supported models').suggestions).toEqual(
    expect.arrayContaining(['Flux .1 D', 'Flux .1 Kontext']),
  );
  settingsSelect.completeAutocomplete('Supported models', '');
  expect(settingsSelect.autocomplete('Supported models').suggestions).toEqual(
    expect.arrayContaining(['Flux .1 D', 'Flux .1 Kontext', 'SD 1.5']),
  );
  expect(settingsSelect.autocomplete('Supported models').suggestions).not.toContain('SDXL');
  settingsSelect.completeAutocomplete('Supported models', 'My Bespoke Model');
  expect(settingsSelect.autocomplete('Supported models').suggestions[0]).toBe('My Bespoke Model');
  settingsSelect.enterAutocomplete('Supported models', [
    'SDXL',
    'Flux .1 D',
    'SD 1.5',
    'My Bespoke Model',
  ]);
  document.querySelector<HTMLButtonElement>('.sugarcubes-cube-editor-metadata__collapse')?.click();
  document.querySelector<HTMLButtonElement>('.sugarcubes-cube-editor-metadata__collapse')?.click();
  expect(settingsSelect.autocomplete('Supported models').values).toEqual([
    'SDXL',
    'Flux .1 D',
    'SD 1.5',
    'My Bespoke Model',
  ]);
  expect(document.querySelector('.sugarcubes-cube-editor-metadata select')).toBeNull();
  expect(document.querySelector('[role="listbox"]')).toBeNull();
  hud.dispose();
});

test('uses Another model as the explicit HUD path for a custom target model', async () => {
  document.body.replaceChildren();
  const settingsSelect = new TestComfySettingsSelectRenderer();
  const save = jest.fn(async () => 'saved' as const);
  const hud = new CubeEditorMetadataHud(
    document,
    {
      canEdit: async () => true,
      save,
      modelSuggestions: () => ['Flux .1 D', 'Flux .1 Kontext'],
    },
    { settingsSelectRenderer: settingsSelect },
  );
  const node = cubeNode();

  await showExpanded(hud, node);

  const target = settingsSelect.single('Target model');
  expect(target.options.map((option) => option.label)).toEqual(
    expect.arrayContaining(['Flux .1 D', 'Flux .1 Kontext', 'Another model…']),
  );
  const anotherModel = target.options.find((option) => option.label === 'Another model…');
  if (!anotherModel) throw new Error('Expected the custom target-model option.');
  settingsSelect.selectSingle('Target model', anotherModel.value);

  const customTarget = document.querySelector<HTMLInputElement>(
    '[data-sugarcubes-focus-key="customTargetModel"]',
  );
  if (!customTarget) throw new Error('Expected the custom target-model input.');
  expect(customTarget.disabled).toBe(false);
  customTarget.value = 'Flux2 Klein';
  customTarget.dispatchEvent(new Event('input', { bubbles: true }));
  expect(settingsSelect.autocomplete('Supported models').values).toEqual(['Flux2 Klein']);

  document.querySelector<HTMLButtonElement>('.sugarcubes-cube-editor-metadata__save')?.click();
  await Promise.resolve();

  expect(save).toHaveBeenCalledWith(
    node,
    expect.objectContaining({
      targetModel: 'Flux2 Klein',
      supportedModels: ['Flux2 Klein'],
    }),
  );
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

  await showExpanded(hud, cubeNode());

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

  await showExpanded(hud, cubeNode());
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

  await showExpanded(hud, cubeNode());

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

  await showExpanded(hud, node);
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

  await showExpanded(hud, node);
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
  const settingsSelect = new TestComfySettingsSelectRenderer();
  const hud = new CubeEditorMetadataHud(
    document,
    {
      canEdit: async () => true,
      save: async () => 'saved',
    },
    {
      settingsSelectRenderer: settingsSelect,
    },
  );
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

  await showExpanded(hud, node);

  const targetModel = document.querySelector<HTMLButtonElement>(
    '.sugarcubes-cube-editor-metadata__target-model [role="combobox"]',
  );
  expect(targetModel?.textContent).toBe('SDXL');
  expect(
    document.querySelector('.sugarcubes-cube-editor-metadata__target-model select'),
  ).toBeNull();
  expect(settingsSelect.autocomplete('Supported models').values).toEqual(['SDXL']);
  const indicator = document.querySelector<HTMLElement>('.sugarcubes-cube-unsaved-indicator');
  expect(indicator?.getAttribute('aria-label')).toBe('Not saved yet');
  expect(indicator?.querySelector('.pi-save')).not.toBeNull();
  expect(indicator?.querySelector('.pi-ban')).not.toBeNull();
  expect(
    document.querySelector<HTMLButtonElement>('.sugarcubes-cube-editor-metadata__save')?.disabled,
  ).toBe(false);
  hud.dispose();
});

test('derives the alias from the native target-model combo and basename-only name', async () => {
  document.body.replaceChildren();
  const settingsSelect = new TestComfySettingsSelectRenderer();
  const hud = new CubeEditorMetadataHud(
    document,
    {
      canEdit: async () => true,
      save: async () => 'saved',
    },
    { settingsSelectRenderer: settingsSelect },
  );
  const node = cubeNode();
  requireCubeIdentity(node).default_alias = 'SDXL/Text to Image';

  await showExpanded(hud, node);

  const name = document.querySelector<HTMLInputElement>(
    `[data-sugarcubes-focus-key="defaultAlias"]`,
  );
  expect(name?.value).toBe('Text to Image');
  expect(
    document.querySelector('.sugarcubes-cube-editor-metadata__titlebar')?.textContent,
  ).toContain('SDXL/Text to Image');

  settingsSelect.selectSingle('Target model', 'Flux');

  expect(
    document.querySelector<HTMLButtonElement>(
      '.sugarcubes-cube-editor-metadata__target-model [role="combobox"]',
    )?.textContent,
  ).toBe('Flux');
  expect(
    document.querySelector('.sugarcubes-cube-editor-metadata__titlebar')?.textContent,
  ).toContain('Flux/Text to Image');
  expect(settingsSelect.autocomplete('Supported models').values).toEqual(['Flux']);
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
    subgraph: {
      id: 'cube-definition',
      name: 'Cube: Untitled Cube',
      _nodes: [],
      inputs: [],
      outputs: [],
    } as never,
    connect() {},
    isSubgraphNode: () => true,
    serialize: () => ({}),
  } as CubeNode;
}

/** Expand a persisted Cube card for tests concerned with its detailed controls. */
async function showExpanded(hud: CubeEditorMetadataHud, node: CubeNode): Promise<void> {
  hud.show(node);
  await Promise.resolve();
  document.querySelector<HTMLButtonElement>('[aria-label="Expand"]')?.click();
  await Promise.resolve();
}
