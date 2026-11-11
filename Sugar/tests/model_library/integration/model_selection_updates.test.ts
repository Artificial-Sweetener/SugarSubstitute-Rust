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
/** Prove rendered model selection checks use exact catalog identities without altering native values. */
import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { mountModelPicker } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerVueRuntime.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import type { ModelPickerBinding } from '../../../frontend/comfyui/ui/model_library/ModelPickerBinding.js';
import type { ModelUpdatePort } from '../../../frontend/comfyui/ui/model_library/ModelUpdates.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';

test.each(['checkpoints', 'diffusion_models', 'loras', 'upscale_models'])(
  'checks selected %s identities while keeping unrelated loader types outside the policy',
  async (kind) => {
    Object.defineProperty(window, 'ResizeObserver', {
      value: ModelFixtureResizeObserver,
      configurable: true,
    });
    const root = document.createElement('div');
    document.body.append(root);
    const values = ['first.safetensors', 'second.safetensors'];
    let current = values[0] ?? '';
    let notify: () => void = () => undefined;
    const write = jest.fn((value: string) => {
      current = value;
      notify();
    });
    const binding: ModelPickerBinding = {
      label: 'model_name',
      kind,
      options: values,
      anchor: root,
      readValue: () => current,
      writeValue: write,
      disabled: () => false,
      subscribe: (listener) => {
        notify = listener;
        return () => {
          notify = () => undefined;
        };
      },
    };
    const items = modelPickerChoices([], kind, values).map((item, index) => ({
      ...item,
      id: (index ? 'b' : 'a').repeat(64),
    }));
    const checkSelection = jest.fn<ModelUpdatePort['checkSelection']>(async () => undefined);
    const updates: ModelUpdatePort = {
      read: () => ({ enabled: true, proposals: [] }),
      subscribe: () => () => undefined,
      load: async () => undefined,
      check: async () => undefined,
      checkSelection,
      setEnabled: async () => undefined,
      dismiss: async () => undefined,
      disableModel: async () => undefined,
    };
    const picker = mountModelPicker(root, binding, items, null, null, updates);
    try {
      await nextTick();
      expect(write).not.toHaveBeenCalled();
      binding.writeValue(values[1] ?? '');
      await nextTick();
      expect(current).toBe(values[1]);
      expect(checkSelection.mock.calls).toEqual(
        kind === 'upscale_models' ? [] : [['a'.repeat(64)], ['b'.repeat(64)]],
      );
    } finally {
      picker.dispose();
      root.remove();
    }
  },
);
