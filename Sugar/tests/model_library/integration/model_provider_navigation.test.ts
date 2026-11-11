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
/** Prove provider navigation follows the clicked artifact and association across menu replacement. */
import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { mountModelPicker } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerVueRuntime.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import type { ModelCatalogItem } from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';
import type { ModelPickerBinding } from '../../../frontend/comfyui/ui/model_library/ModelPickerBinding.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';

/** Resolve an exact displayed command instead of relying on provider order or DOM position. */
function command(label: string): HTMLButtonElement {
  const row = [...document.querySelectorAll<HTMLButtonElement>('[role=menuitem]')].find(
    (item) => item.textContent === label,
  );
  if (!row) throw new Error(`Missing provider command: ${label}`);
  return row;
}

test('same-name cards and multiple provider links open their exact page without changing the model', async () => {
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
  const root = document.createElement('div');
  document.body.append(root);
  const values = ['first.pth', 'second.pth'];
  const choices = modelPickerChoices([], 'upscale_models', values);
  const first = choices[0];
  const second = choices[1];
  if (!first || !second) throw new Error('Missing native model choices.');
  const models: readonly ModelCatalogItem[] = [
    {
      ...first,
      id: 'a'.repeat(64),
      title: 'Same display name',
      provider: 'openmodeldb',
      providerUrl: 'https://openmodeldb.info/models/4x-Remacri',
      providerLinks: [
        {
          providerId: 'openmodeldb',
          providerName: 'OpenModelDB',
          modelId: '4x-Remacri',
          versionId: 'first-artifact',
          modelPageUrl: 'https://openmodeldb.info/models/4x-Remacri',
        },
        {
          providerId: 'civitai',
          providerName: 'CivitAI',
          modelId: '1001',
          versionId: '2001',
          modelPageUrl: 'https://civitai.com/models/1001',
        },
      ],
    },
    {
      ...second,
      id: 'b'.repeat(64),
      title: 'Same display name',
      provider: 'civitai',
      providerUrl: 'https://civitai.com/models/1002',
      providerModelId: '1002',
      providerVersionId: '2002',
    },
  ];
  const write = jest.fn<ModelPickerBinding['writeValue']>();
  const binding: ModelPickerBinding = {
    label: 'Upscaler',
    kind: 'upscale_models',
    options: values,
    anchor: root,
    readValue: () => first.value,
    writeValue: write,
    disabled: () => false,
    subscribe: () => () => undefined,
  };
  const navigate = jest.spyOn(window, 'open').mockReturnValue(null);
  const picker = mountModelPicker(root, binding, models);
  try {
    const field = root.querySelector('input');
    if (!field) throw new Error('Missing native model field.');
    field.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    await nextTick();
    command('Go to OpenModelDB page').click();
    await nextTick();
    expect(navigate).toHaveBeenLastCalledWith(
      'https://openmodeldb.info/models/4x-Remacri',
      '_blank',
      'noopener,noreferrer',
    );
    field.click();
    await nextTick();
    const firstTile = document.querySelector('[data-model-value="first.pth"]');
    const secondTile = document.querySelector('[data-model-value="second.pth"]');
    if (!firstTile || !secondTile) throw new Error('Missing same-name model tiles.');
    firstTile.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    await nextTick();
    secondTile.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    await nextTick();
    expect(document.querySelectorAll('[role=menu]')).toHaveLength(1);
    command('Go to CivitAI page').click();
    await nextTick();
    expect(navigate).toHaveBeenLastCalledWith(
      'https://civitai.com/models/1002',
      '_blank',
      'noopener,noreferrer',
    );
    firstTile.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    await nextTick();
    command('Go to CivitAI page').click();
    await nextTick();
    expect(navigate).toHaveBeenLastCalledWith(
      'https://civitai.com/models/1001',
      '_blank',
      'noopener,noreferrer',
    );
    expect(navigate).toHaveBeenCalledTimes(3);
    expect(write).not.toHaveBeenCalled();
    expect(binding.readValue()).toBe(first.value);
    expect(document.querySelector('[role=menu]')).toBeNull();
  } finally {
    picker.dispose();
    navigate.mockRestore();
    root.remove();
  }
});
