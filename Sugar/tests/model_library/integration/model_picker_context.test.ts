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
/** Prove banners and tiles share context menus without entering details or changing native values. */
import { nextTick } from 'vue';
import { jest } from '@jest/globals';
import { mountModelPicker } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerVueRuntime.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import type { ModelPickerBinding } from '../../../frontend/comfyui/ui/model_library/ModelPickerBinding.js';
import type { ModelPickerActions } from '../../../frontend/comfyui/ui/model_library/ModelPickerActions.js';

import { ModelFixtureResizeObserver } from '../support/resize_observer.js';

test('banner and wall menus have identical actions and preserve selection through dismissal and refresh', async () => {
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
  const root = document.createElement('div');
  document.body.append(root);
  const writes: string[] = [];
  const value = 'Anima\\test.safetensors';
  const binding: ModelPickerBinding = {
    label: 'diffusion_model',
    kind: 'diffusion_models',
    options: [value],
    anchor: root,
    readValue: () => value,
    writeValue: (next) => {
      writes.push(next);
    },
    disabled: () => false,
    subscribe: () => () => undefined,
  };
  const base = modelPickerChoices([], 'diffusion_models', [value])[0];
  if (!base) throw new Error('Missing fixture model.');
  const refresh = jest.fn<ModelPickerActions['refreshMetadata']>(async (_id, progress) => {
    progress('Metadata refreshed.');
  });
  const actions: ModelPickerActions = {
    credentials: {
      status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
    },
    classifyContent: async () => undefined,
    refreshMetadata: refresh,
    replaceThumbnail: async () => undefined,
    releases: async () => [],
    acquire: async () => '21.safetensors',
  };
  const feedback = jest.fn();
  const picker = mountModelPicker(
    root,
    binding,
    [
      {
        ...base,
        id: 'a'.repeat(64),
        provider: 'civitai',
        providerUrl: 'https://civitai.com/models/123',
      },
    ],
    actions,
    feedback,
  );
  const field = root.querySelector('input');
  if (!field) throw new Error('Missing model field.');
  try {
    const prepared = picker.contextEntries();
    expect(prepared.map((entry) => entry.label)).toEqual([
      'Go to CivitAI page',
      'Refresh CivitAI metadata',
      'Set thumbnail from canvas',
    ]);
    field.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    await nextTick();
    const labels = [...document.querySelectorAll('[role=menuitem]')].map(
      (item) => item.textContent,
    );
    expect(labels).toEqual([
      'Go to CivitAI page',
      'Refresh CivitAI metadata',
      'Set thumbnail from canvas',
    ]);
    expect(document.querySelector('[role=menuitem][aria-disabled=true]')?.textContent).toBe(
      'Set thumbnail from canvas',
    );
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await nextTick();
    field.click();
    await nextTick();
    const tile = document.querySelector('.sc-model-tile');
    if (!tile) throw new Error('Missing tile.');
    tile.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    await nextTick();
    expect(
      [...document.querySelectorAll('[role=menuitem]')].map((item) => item.textContent),
    ).toEqual(labels);
    expect(document.querySelector('.sc-model-details')).toBeNull();
    expect(document.querySelector('.sc-model-wall')).not.toBeNull();
    const button = document.querySelector<HTMLButtonElement>('[role=menuitem]:nth-child(2)');
    if (!button) throw new Error('Missing refresh command.');
    button.click();
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    await nextTick();
    expect(refresh).toHaveBeenCalledTimes(1);
    expect(feedback).toHaveBeenCalledWith('info', 'Metadata refreshed.');
    expect(writes).toEqual([]);
    expect(binding.readValue()).toBe(value);
    expect(document.querySelector('[role=menu]')).toBeNull();
  } finally {
    picker.dispose();
    root.remove();
  }
});
