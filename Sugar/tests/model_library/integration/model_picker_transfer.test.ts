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
/** Prove shared field progress survives remounts and never commits cancelled or failed downloads. */
import { nextTick } from 'vue';
import { modelPickerCss } from '../../../frontend/comfyui/ui/model_library/ModelPickerStyles.js';
import { nativeModelFieldFixture } from '../support/native_model_field.js';
import { ModelPickerSessions } from '../../../frontend/comfyui/ui/model_library/ModelPickerSessions.js';
import { readComfyModelFields } from '../../../frontend/comfyui/ui/model_library/ComfyModelFieldAdapter.js';
import { mountModelPicker } from '../../../frontend/comfyui/ui/model_library/vue/ModelPickerVueRuntime.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import type { ModelTransferObserver } from '../../../frontend/comfyui/ui/model_library/ModelTransfer.js';

test('preserves numeric progress in a replacement combobox and resolves refreshed options at commit', async () => {
  const native = nativeModelFieldFixture();
  const nativeMount = native.card.parentElement;
  const binding = readComfyModelFields(native.card, native.snapshot)[0];
  if (!binding) throw new Error('Missing native field.');
  const sessions = new ModelPickerSessions(document, () => native.snapshot, {
    graph: native.graph,
  });
  const session = sessions.get(native.card, binding);
  const root = document.createElement('div');
  native.card.append(root);
  const items = modelPickerChoices([], binding.kind, binding.options);
  let picker = mountModelPicker(root, binding, items, null, null, null, null, null, null, session);
  let finish: (filename: string) => void = () => {
    throw new Error('Download has not begun.');
  };
  const style = document.createElement('style');
  style.textContent = modelPickerCss;
  document.head.append(style);
  const operation = session.useDownload(
    async (progress) => {
      progress({ message: '<b>Downloading</b>', percentage: 25 });
      return new Promise<string>((resolve) => {
        finish = resolve;
      });
    },
    { name: 'Hassaku <img>', previewUrl: '/version-preview' },
  );
  try {
    await nextTick();
    expect(root.querySelector('[role=progressbar]')?.getAttribute('aria-valuenow')).toBe('25');
    expect(root.querySelector('input')?.value).toBe('Downloading Hassaku <img>');
    expect(root.querySelector('.sc-model-transfer-caption')?.textContent).toBe(
      'DownloadingHassaku <img>',
    );
    expect(root.querySelector('.sc-model-transfer-thumbnail')?.getAttribute('src')).toBe(
      '/version-preview',
    );
    const field = root.querySelector('.sc-model-field');
    if (!field) throw new Error('Missing transferring field.');
    expect(getComputedStyle(field).opacity).toBe('1');
    const fill = root.querySelector('[role=progressbar]');
    if (!fill) throw new Error('Missing transfer paint.');
    expect(getComputedStyle(fill).opacity).toBe('0.55');
    expect(root.querySelector('b')).toBeNull();
    const caption = root.querySelector('.sc-model-transfer-caption');
    if (!caption) throw new Error('Missing transfer caption.');
    expect(getComputedStyle(caption).fontSize).toBe('0.9em');
    expect(getComputedStyle(caption).lineHeight).toBe('1.05');
    const input = root.querySelector('input');
    if (!input) throw new Error('Missing accessible combo.');
    expect(input.title).toBe('Hassaku <img>');
    expect(getComputedStyle(input).color).toBe('transparent');
    root.querySelector('.sc-model-transfer-thumbnail')?.dispatchEvent(new Event('error'));
    await nextTick();
    expect(root.querySelector('.sc-model-transfer-thumbnail')).toBeNull();
    expect(root.querySelector('[role=progressbar]')).not.toBeNull();
    expect(root.querySelector('.sc-model-transfer-caption img')).toBeNull();
    picker.dispose();
    native.props.widget.options = { values: ['folder\\new.safetensors'] };
    const refreshed = readComfyModelFields(native.card, native.snapshot)[0];
    if (!refreshed) throw new Error('Missing refreshed field.');
    expect(sessions.get(native.card, refreshed)).toBe(session);
    picker = mountModelPicker(root, refreshed, items, null, null, null, null, null, null, session);
    expect(root.querySelector('[role=progressbar]')?.getAttribute('aria-valuenow')).toBe('25');
    expect(root.querySelector('.sc-model-transfer-thumbnail')?.getAttribute('src')).toBe(
      '/version-preview',
    );
    finish('folder/new.safetensors');
    await operation;
    await nextTick();
    expect(native.emit).toHaveBeenCalledWith('update:modelValue', 'folder\\new.safetensors');
    expect(root.querySelector('[role=progressbar]')).toBeNull();
  } finally {
    style.remove();
    picker.dispose();
    sessions.clear();
    nativeMount?.remove();
  }
});

test.each(['cancelled', 'failed', 'removed'] as const)(
  'clears a %s transfer, ignores late progress, and preserves the model',
  async (ending) => {
    const native = nativeModelFieldFixture();
    const nativeMount = native.card.parentElement;
    const binding = readComfyModelFields(native.card, native.snapshot)[0];
    if (!binding) throw new Error('Missing native field.');
    const sessions = new ModelPickerSessions(document, () => native.snapshot, {
      graph: native.graph,
    });
    const session = sessions.get(native.card, binding);
    const controller = new AbortController();
    let late: ModelTransferObserver = () => undefined;
    let finish: () => void = () => undefined;
    const transferSignal: { value: AbortSignal | null } = { value: null };
    const operation = session.download(
      async (progress, signal) => {
        transferSignal.value = signal;
        late = progress;
        progress({ message: 'Downloading', percentage: null });
        await new Promise<void>((resolve) => {
          finish = resolve;
        });
        if (ending === 'failed') throw new Error('Provider unavailable');
        return 'new.safetensors';
      },
      { name: 'New model', previewUrl: null },
      controller.signal,
    );
    const observed = operation.catch((error: unknown) => error);
    if (ending === 'cancelled') controller.abort();
    if (ending === 'removed') {
      native.graph._nodes.splice(0);
      native.card.remove();
      sessions.reconcile();
    }
    expect(transferSignal.value).not.toBeNull();
    expect(transferSignal.value?.aborted).toBe(ending !== 'failed');
    finish();
    const result: unknown = await observed;
    expect(result).toBeInstanceOf(Error);
    late({ message: 'Late progress', percentage: 99 });
    expect(session.read()).toBeNull();
    expect(native.props.modelValue).toBe('Flux\\alpha.safetensors');
    expect(native.emit).not.toHaveBeenCalled();
    sessions.clear();
    nativeMount?.remove();
  },
);

test('closes accepted versions immediately, retains field progress, and selects only the refreshed installed filename', async () => {
  const native = nativeModelFieldFixture();
  const nativeMount = native.card.parentElement;
  const binding = readComfyModelFields(native.card, native.snapshot)[0];
  if (!binding) throw new Error('Missing native field.');
  const sessions = new ModelPickerSessions(document, () => native.snapshot, {
    graph: native.graph,
  });
  const session = sessions.get(native.card, binding);
  const root = document.createElement('div');
  native.card.append(root);
  const items = modelPickerChoices([], binding.kind, binding.options);
  const item = items[0];
  if (!item) throw new Error('Missing origin model.');
  const picker = mountModelPicker(
    root,
    binding,
    items,
    null,
    null,
    null,
    null,
    null,
    null,
    session,
  );
  let finish: (filename: string) => void = () => {
    throw new Error('No acquisition.');
  };
  const request: { signal: AbortSignal | null } = { signal: null };
  const browsing = session.versions(
    item,
    {
      credentials: {
        status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
        save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      },
      classifyContent: async () => undefined,
      refreshMetadata: async () => undefined,
      replaceThumbnail: async () => undefined,
      releases: async () => [
        {
          versionId: 'remote',
          name: 'Hassaku v3.4',
          baseModel: 'SDXL',
          publishedAt: '',
          pageUrl: '',
          previewUrl: null,
          onDisk: false,
          inUse: false,
          files: [
            {
              fileId: 'file',
              fileName: 'new.safetensors',
              sha256: 'a'.repeat(64),
              sizeBytes: 100,
              kind: 'checkpoints',
              access: 'public',
            },
          ],
        },
      ],
      acquire: async (_model, _version, _file, _status, signal, transfer) => {
        request.signal = signal;
        transfer?.({ message: 'Downloading Hassaku', percentage: 40 });
        return new Promise<string>((resolve) => {
          finish = resolve;
        });
      },
    },
    () => {
      throw new Error('Unexpected acquisition failure.');
    },
  );
  try {
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    await nextTick();
    document.querySelector<HTMLButtonElement>('.sc-model-version-card')?.click();
    await nextTick();
    document
      .querySelector<HTMLButtonElement>(
        '.sc-model-version-modal .sugarcubes-modal__footer-actions button:last-child',
      )
      ?.click();
    await browsing;
    await nextTick();
    expect(document.querySelector('.sc-model-version-modal')).toBeNull();
    expect(request.signal?.aborted).toBe(false);
    expect(root.querySelector('[role=progressbar]')?.getAttribute('aria-valuenow')).toBe('40');
    expect(native.emit).not.toHaveBeenCalled();
    native.props.widget.options = { values: ['Hassaku\\new.safetensors'] };
    finish('Hassaku/new.safetensors');
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    await nextTick();
    expect(native.props.modelValue).toBe('Hassaku\\new.safetensors');
    expect(native.emit).toHaveBeenCalledTimes(1);
    expect(session.read()).toBeNull();
    expect(root.querySelector('[role=progressbar]')).toBeNull();
  } finally {
    picker.dispose();
    sessions.clear();
    nativeMount?.remove();
    await browsing;
  }
});
