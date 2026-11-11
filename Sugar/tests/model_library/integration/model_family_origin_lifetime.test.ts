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
/** Prove originating-field removal aborts its family work without leaking into a replacement renderer. */
import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { mountModelPicker } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerVueRuntime.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import type { ModelPickerBinding } from '../../../frontend/comfyui/ui/model_library/ModelPickerBinding.js';
import type { ModelPickerActions } from '../../../frontend/comfyui/ui/model_library/ModelPickerActions.js';
import type { ModelRelease } from '../../../frontend/comfyui/ui/model_library/ModelReleases.js';
import type { ModelUpdatePort } from '../../../frontend/comfyui/ui/model_library/ModelUpdates.js';
import type { ModelTransferObserver } from '../../../frontend/comfyui/ui/model_library/ModelTransfer.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';

/** Let modal cleanup and Vue teardown settle across their promise boundaries. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  await nextTick();
}

/** Build eligible, already validated release evidence without contacting a provider. */
function release(versionId: string): ModelRelease {
  return {
    versionId,
    name: `Fixture v${versionId}`,
    baseModel: 'SDXL',
    publishedAt: '',
    pageUrl: 'https://civitai.com/models/123',
    previewUrl: null,
    onDisk: versionId === '20',
    inUse: versionId === '20',
    files: [
      {
        access: 'public',
        fileId: versionId,
        fileName: `${versionId}.safetensors`,
        sha256: 'b'.repeat(64),
        sizeBytes: 10,
        kind: 'checkpoints',
      },
    ],
  };
}

test.each(['history', 'download'] as const)(
  'aborts pending %s when the originating picker unmounts and keeps a replacement field clean',
  async (operation) => {
    Object.defineProperty(window, 'ResizeObserver', {
      value: ModelFixtureResizeObserver,
      configurable: true,
    });
    const root = document.createElement('div');
    document.body.append(root);
    const value = 'installed.safetensors';
    const writeValue = jest.fn<ModelPickerBinding['writeValue']>();
    const unsubscribeBinding = jest.fn();
    const unsubscribeUpdates = jest.fn();
    const binding: ModelPickerBinding = {
      label: 'ckpt_name',
      kind: 'checkpoints',
      options: [value],
      anchor: root,
      readValue: () => value,
      writeValue,
      disabled: () => false,
      subscribe: () => unsubscribeBinding,
    };
    const base = modelPickerChoices([], 'checkpoints', [value])[0];
    if (!base) throw new Error('Missing installed model.');
    const item = { ...base, id: 'a'.repeat(64), providerUrl: 'https://civitai.com/models/123' };
    const updates: ModelUpdatePort = {
      read: () => ({
        enabled: true,
        proposals: [
          { modelId: item.id, sha256: 'b'.repeat(64), modelName: item.title, versionId: '21' },
        ],
      }),
      subscribe: () => unsubscribeUpdates,
      load: async () => undefined,
      check: async () => undefined,
      checkSelection: async () => undefined,
      setEnabled: async () => undefined,
      dismiss: async () => undefined,
      disableModel: async () => undefined,
    };
    const request: {
      signal: AbortSignal | null;
      progress: ModelTransferObserver | null;
    } = { signal: null, progress: null };
    /** Keep only the external operation pending until its real owner cancels it. */
    const pending = (signal: AbortSignal): Promise<never> => {
      request.signal = signal;
      return new Promise((_resolve, reject) => {
        signal.addEventListener('abort', () => reject(new Error('Origin removed.')), {
          once: true,
        });
      });
    };
    const actions: ModelPickerActions = {
      credentials: {
        status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
        save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      },
      classifyContent: async () => undefined,
      refreshMetadata: async () => undefined,
      replaceThumbnail: async () => undefined,
      releases: async (_id, signal) =>
        operation === 'history' ? pending(signal) : [release('20'), release('21')],
      acquire: async (_id, _versionId, _fileId, _progress, signal, transfer) => {
        request.progress = transfer ?? null;
        transfer?.({ message: 'Downloading 25%', percentage: 25 });
        return pending(signal);
      },
    };
    const feedback = jest.fn();
    const logging = jest.spyOn(console, 'error').mockImplementation(() => undefined);
    const picker = mountModelPicker(root, binding, [item], actions, feedback, updates);
    let replacement: ReturnType<typeof mountModelPicker> | null = null;
    try {
      root.querySelector<HTMLButtonElement>('.sc-model-update')?.click();
      await settle();
      expect(document.querySelector('.sc-model-version-modal')).not.toBeNull();
      if (operation === 'download') {
        document
          .querySelector<HTMLButtonElement>('.sc-model-version-card:not([data-in-use=true])')
          ?.click();
        await nextTick();
        document
          .querySelector<HTMLButtonElement>(
            '.sc-model-version-modal .sugarcubes-modal__footer-actions button:last-child',
          )
          ?.click();
        await settle();
        expect(document.querySelector('.sc-model-version-modal')).toBeNull();
        expect(root.querySelector('[role=progressbar]')?.getAttribute('aria-valuenow')).toBe('25');
        expect(root.querySelector('input')?.value).toBe('Downloading installed');
        expect(root.querySelector('.sc-model-transfer-caption')?.textContent).toBe(
          'Downloadinginstalled',
        );
        expect(document.querySelector('.sc-model-version-status')).toBeNull();
      }
      const signal = request.signal;
      if (!signal) throw new Error('Missing pending operation.');
      expect(signal.aborted).toBe(false);
      picker.dispose();
      await settle();
      expect(signal.aborted).toBe(true);
      expect(document.querySelector('.sc-model-version-overlay')).toBeNull();
      expect(document.querySelector('.sc-model-version-modal')).toBeNull();
      expect(unsubscribeBinding).toHaveBeenCalledTimes(1);
      expect(unsubscribeUpdates).toHaveBeenCalledTimes(1);
      replacement = mountModelPicker(root, binding, [item], actions, feedback, updates);
      const report = request.progress;
      if (report) report({ message: 'Late progress from removed origin', percentage: 75 });
      await settle();
      expect(document.querySelector('.sc-model-version-overlay')).toBeNull();
      expect(root.textContent).not.toContain('Late progress');
      expect(writeValue).not.toHaveBeenCalled();
      expect(binding.readValue()).toBe(value);
      expect(feedback).not.toHaveBeenCalled();
      expect(logging).not.toHaveBeenCalled();
    } finally {
      replacement?.dispose();
      if (!replacement) picker.dispose();
      root.remove();
      logging.mockRestore();
    }
  },
);
