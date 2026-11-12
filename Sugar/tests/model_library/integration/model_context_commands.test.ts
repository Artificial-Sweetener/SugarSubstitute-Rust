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
/** Prove header contributions and update-icon menus retain exact model command ownership. */
import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { mountModelPicker } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerVueRuntime.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import type { ModelPickerActions } from '../../../frontend/comfyui/ui/model_library/ModelPickerActions.js';
import type { ModelUpdatePort } from '../../../frontend/comfyui/ui/model_library/ModelUpdates.js';
import { ModelFixtureResizeObserver } from '../support/resize_observer.js';

/** Mount the production picker with only provider transport and update persistence replaced. */
function fixture(hasUpdate = true, versionCount = 2) {
  Object.defineProperty(window, 'ResizeObserver', {
    value: ModelFixtureResizeObserver,
    configurable: true,
  });
  const root = document.createElement('div');
  document.body.append(root);
  const value = 'local.safetensors';
  const base = modelPickerChoices([], 'checkpoints', [value])[0];
  if (!base) throw new Error('Missing model choice.');
  const item = {
    ...base,
    id: 'a'.repeat(64),
    provider: 'civitai',
    providerModelId: '123',
    providerVersionId: '10',
    baseModel: 'SDXL 1.0',
    versionCount,
    providerUrl: 'https://civitai.com/models/123',
  };
  const writes = jest.fn();
  const releases = jest.fn<ModelPickerActions['releases']>(async () => []);
  const classifyContent = jest.fn<ModelPickerActions['classifyContent']>(async () => undefined);
  const dismiss = jest.fn<ModelUpdatePort['dismiss']>(async () => undefined);
  const disableModel = jest.fn<ModelUpdatePort['disableModel']>(async () => undefined);
  const updates: ModelUpdatePort = {
    read: () => ({
      enabled: hasUpdate,
      proposals: hasUpdate
        ? [{ modelId: item.id, sha256: 'b'.repeat(64), modelName: '<b>Local</b>', versionId: '20' }]
        : [],
    }),
    subscribe: () => () => undefined,
    load: async () => undefined,
    check: async () => undefined,
    checkSelection: async () => undefined,
    setEnabled: async () => undefined,
    dismiss,
    disableModel,
  };
  const picker = mountModelPicker(
    root,
    {
      label: 'ckpt_name',
      kind: 'checkpoints',
      options: [value],
      anchor: root,
      readValue: () => value,
      writeValue: writes,
      disabled: () => false,
      subscribe: () => () => undefined,
    },
    [item],
    {
      credentials: {
        status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
        save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      },
      releases,
      acquire: async () => '21.safetensors',
      classifyContent,
      refreshMetadata: async () => undefined,
      replaceThumbnail: async () => undefined,
    },
    null,
    updates,
  );
  return {
    credentials: {
      status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
    },
    root,
    picker,
    item,
    writes,
    releases,
    classifyContent,
    dismiss,
    disableModel,
  };
}

/** Settle asynchronous modal and command boundaries through their event-loop turn. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  await nextTick();
}

test.each([true, false])(
  'other versions opens the exact family with update availability %s',
  async (hasUpdate) => {
    const current = fixture(hasUpdate);
    try {
      const entry = current.picker
        .contextEntries()
        .find((entry) => entry.label === 'See other versions');
      expect(
        current.picker.contextEntries().some((entry) => entry.label === 'Content classification'),
      ).toBe(false);
      expect(entry).toBeDefined();
      expect(
        current.picker
          .contextEntries()
          .slice(-2)
          .map((item) => item.label),
      ).toEqual(['See other versions', 'Set thumbnail from canvas']);
      entry?.select?.();
      await settle();
      expect(current.releases).toHaveBeenCalledWith(current.item.id, expect.any(AbortSignal));
      expect(document.querySelector('.sc-model-version-modal')).not.toBeNull();
      expect(current.writes).not.toHaveBeenCalled();
      expect(current.classifyContent).not.toHaveBeenCalled();
    } finally {
      current.picker.dispose();
      current.root.remove();
    }
    await settle();
    expect(document.querySelector('.sc-model-version-modal')).toBeNull();
  },
);

test.each([false, true])(
  'hides single-version history from field menus with update availability %s',
  async (hasUpdate) => {
    const current = fixture(hasUpdate, 1);
    try {
      expect(
        current.picker.contextEntries().some((entry) => entry.label === 'See other versions'),
      ).toBe(false);
      current.root
        .querySelector('input')
        ?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
      await nextTick();
      expect(
        [...document.querySelectorAll('[role=menuitem]')].map((entry) => entry.textContent),
      ).not.toContain('See other versions');
      expect(current.releases).not.toHaveBeenCalled();
      current.picker.update([{ ...current.item, versionCount: 2 }]);
      await nextTick();
      expect(
        current.picker.contextEntries().some((entry) => entry.label === 'See other versions'),
      ).toBe(true);
    } finally {
      current.picker.dispose();
      current.root.remove();
    }
  },
);

test.each(['dismiss', 'disableModel'] as const)(
  'update-icon %s routes only to the captured model and renders its name literally',
  async (command) => {
    const current = fixture();
    try {
      current.root
        .querySelector('.sc-model-update')
        ?.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
      await nextTick();
      const entries = [...document.querySelectorAll<HTMLButtonElement>('[role=menuitem]')];
      expect(entries.map((entry) => entry.textContent)).toEqual([
        'Dismiss',
        "Don't check for updates for <b>Local</b>",
      ]);
      expect(document.querySelector('[role=menu] b')).toBeNull();
      entries[command === 'dismiss' ? 0 : 1]?.click();
      await settle();
      expect(current[command]).toHaveBeenCalledWith(current.item.id);
      expect(current[command === 'dismiss' ? 'disableModel' : 'dismiss']).not.toHaveBeenCalled();
      expect(current.writes).not.toHaveBeenCalled();
      expect(current.releases).not.toHaveBeenCalled();
      expect(document.querySelector('[role=menu]')).toBeNull();
    } finally {
      current.picker.dispose();
      current.root.remove();
    }
  },
);
