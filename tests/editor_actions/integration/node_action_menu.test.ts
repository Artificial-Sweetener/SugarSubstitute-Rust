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
/** Prove node menus share dimensions, preserve literal labels and apply through field owners. */
import { expect, test } from '@jest/globals';
import { nodeActionEntries } from '../../../frontend/comfyui/ui/editor_actions/presentation/NodeActionMenu.js';
import { UserPresetLibrary } from '../../../frontend/comfyui/ui/editor_actions/application/UserPresetLibrary.js';
import { SaveNodePresetDialog } from '../../../frontend/comfyui/ui/editor_actions/presentation/SaveNodePresetDialog.js';
import { GLOBAL_ASSOCIATION } from '../../../frontend/comfyui/ui/editor_actions/domain/UserPreset.js';
import type { NativeEditorCard } from '../../../frontend/comfyui/ui/editor_actions/infrastructure/NativeEditorCardAdapter.js';
test('orders presets before field actions and advanced visibility, preserving values until invoked', async () => {
  const library = new UserPresetLibrary(
    { load: async () => [], save: async () => undefined },
    () => 'id',
    () => 'now',
  );
  await library.prepare();
  await library.saveNode('<b>Wide</b>', 'Latent', { width: 1600 }, GLOBAL_ASSOCIATION);
  let width = 1024;
  const card: NativeEditorCard = {
    modelInputs: [],
    root: document.createElement('div'),
    header: null,
    advanced: { shown: () => false, setShown: () => undefined },
    title: 'Latent',
    nodeType: 'Latent',
    fields: [
      {
        name: 'width',
        kind: 'number',
        element: document.createElement('div'),
        read: () => width,
        disabled: () => false,
        write: (value) => {
          if (typeof value !== 'number') return false;
          width = value;
          return true;
        },
      },
      {
        name: 'height',
        kind: 'number',
        element: document.createElement('div'),
        read: () => 1024,
        disabled: () => false,
        write: () => true,
      },
    ],
  };
  const entries = nodeActionEntries(
    card,
    library,
    [],
    new SaveNodePresetDialog(document),
    () => undefined,
    () => undefined,
  );
  expect(entries.filter((entry) => !entry.separator).map((entry) => entry.label)).toEqual([
    'Apply preset',
    'Save current Latent as preset...',
    'Swap width & height',
    'Set ratio by Width',
    'Set ratio by Height',
    'Save current dimensions',
    'Show advanced inputs',
  ]);
  expect(width).toBe(1024);
  entries[0]?.children?.[0]?.select?.();
  expect(width).toBe(1600);
});
test('save dialog names and scopes stay together and cancellation does not produce a preset', async () => {
  const dialog = new SaveNodePresetDialog(document);
  const result = dialog.open('Save Latent preset', [GLOBAL_ASSOCIATION]);
  const name = document.querySelector<HTMLInputElement>('.sc-save-node-preset input');
  expect(document.querySelector('.sc-save-node-preset select')?.textContent).toBe('Global');
  if (!name) throw new Error('Missing name field');
  name.value = '<img src=x>';
  name.dispatchEvent(new Event('input', { bubbles: true }));
  expect(document.querySelector('img')).toBeNull();
  dialog.close();
  expect(await result).toBeNull();
});

test('retains authored field contribution order instead of grouping all dimensions before models', async () => {
  const library = new UserPresetLibrary(
    { load: async () => [], save: async () => undefined },
    () => 'id',
    () => 'now',
  );
  await library.prepare();
  const card: NativeEditorCard = {
    modelInputs: [],
    root: document.createElement('div'),
    header: null,
    advanced: null,
    title: 'Generation',
    nodeType: 'Generation',
    fields: ['ckpt_name', 'width', 'height', 'vae_name'].map((name) => ({
      name,
      kind: name.endsWith('name') ? 'combo' : 'number',
      element: document.createElement('div'),
      read: () => (name.endsWith('name') ? 'auto' : 1024),
      disabled: () => false,
      write: () => true,
    })),
  };
  const entries = nodeActionEntries(
    card,
    library,
    [],
    new SaveNodePresetDialog(document),
    () => undefined,
    () => undefined,
    [
      { fieldName: 'vae_name', entries: [{ label: 'VAE metadata' }] },
      { fieldName: 'ckpt_name', entries: [{ label: 'Checkpoint metadata' }] },
    ],
  );
  expect(entries.filter((entry) => !entry.separator).map((entry) => entry.label)).toEqual([
    'Save current Generation as preset...',
    'Checkpoint metadata',
    'Swap width & height',
    'Set ratio by Width',
    'Set ratio by Height',
    'Save current dimensions',
    'VAE metadata',
  ]);
});
