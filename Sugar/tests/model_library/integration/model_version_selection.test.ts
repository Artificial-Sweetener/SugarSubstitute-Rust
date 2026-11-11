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
/** Prove version acceptance uses exact local filenames and commits only successful acquisitions. */
import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { ModelVersionDialog } from '../../../frontend/comfyui/ui/model_library/ModelVersionDialog.js';
import { ModelPickerSession } from '../../../frontend/comfyui/ui/model_library/ModelPickerSession.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import type { ModelRelease } from '../../../frontend/comfyui/ui/model_library/ModelReleases.js';
import type { ModelPickerActions } from '../../../frontend/comfyui/ui/model_library/ModelPickerActions.js';
import { modelPickerCss } from '../../../frontend/comfyui/ui/model_library/ModelPickerStyles.js';

/** Drain provider and Vue transitions without depending on real network timing. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  await nextTick();
}

test('keeps exactly one initial choice, permits returning to the current version, and uses newest-first compact cards', async () => {
  const item = modelPickerChoices([], 'checkpoints', ['current.safetensors'])[0];
  if (!item) throw new Error('Missing model.');
  const acquire = jest.fn<ModelPickerActions['acquire']>();
  const select = jest.fn<(filename: string) => void>();
  const versions: readonly ModelRelease[] = ['10', '20', '30'].map((versionId) => ({
    versionId,
    name: versionId,
    baseModel: 'SDXL',
    publishedAt: '',
    pageUrl: 'https://civitai.com/models/1',
    previewUrl: null,
    onDisk: versionId !== '30',
    inUse: versionId === '20',
    installedFileName:
      versionId === '20' ? item.value : versionId === '10' ? '10.safetensors' : null,
    files: [
      {
        access: 'public',
        fileId: versionId,
        fileName: `${versionId}.safetensors`,
        sha256: 'b'.repeat(64),
        sizeBytes: 1024,
        kind: 'checkpoints',
      },
    ],
  }));
  const dialog = new ModelVersionDialog(
    document,
    item,
    {
      credentials: {
        status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
        save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      },
      classifyContent: async () => undefined,
      refreshMetadata: async () => undefined,
      replaceThumbnail: async () => undefined,
      releases: async () => versions,
      acquire,
    },
    {
      current: () => item.value,
      select,
      download: (operation, _model, signal) =>
        operation(() => undefined, signal ?? new AbortController().signal),
      useDownload: async (operation) => {
        await operation(() => undefined, new AbortController().signal);
      },
    },
    () => undefined,
  );
  const style = document.createElement('style');
  style.textContent = modelPickerCss;
  const closed = dialog.open();
  try {
    await settle();
    document.head.append(style);
    const cards = Array.from(
      document.querySelectorAll<HTMLButtonElement>('.sc-model-version-card'),
    );
    const current = cards[1];
    const oldest = cards[2];
    const newest = cards[0];
    const accept = document.querySelector<HTMLButtonElement>(
      '.sc-model-version-modal .sugarcubes-modal__footer-actions button:last-child',
    );
    if (!current || !newest || !oldest || !accept) throw new Error('Missing selector controls.');
    expect(cards.map((card) => card.querySelector('.sc-model-version-name')?.textContent)).toEqual([
      '30',
      '20',
      '10',
    ]);
    expect(cards.every((card) => card.querySelector('.sc-model-version-check'))).toBe(true);
    expect(current.disabled).toBe(false);
    expect(current.getAttribute('aria-pressed')).toBe('true');
    expect(accept.disabled).toBe(false);
    expect(accept.textContent).toBe('Use version');
    expect(document.querySelector('.sc-model-version-availability')).toBeNull();
    expect(current.querySelector('.sc-model-version-portrait')?.textContent).not.toContain(
      'In use',
    );
    expect(current.querySelector('.sc-model-version-state')?.textContent).toBe('On disk');
    expect(oldest.querySelector('.sc-model-version-state')?.textContent).toBe('On disk');
    expect(newest.querySelector('.sc-model-version-state')?.textContent).toBe('1 KiB');
    newest.click();
    await nextTick();
    expect(accept.querySelectorAll('.sc-model-action-icon')).toHaveLength(1);
    expect(
      accept
        .querySelector<HTMLElement>('.sc-model-action-icon')
        ?.style.getPropertyValue('--sc-model-action-icon'),
    ).toContain('/icons/model/download.svg');
    expect(current.getAttribute('aria-pressed')).toBe('false');
    expect(document.querySelectorAll('.sc-model-version-selected')).toHaveLength(1);
    expect(getComputedStyle(current).borderColor).toBe(getComputedStyle(oldest).borderColor);
    current.click();
    await nextTick();
    expect(newest.getAttribute('aria-pressed')).toBe('false');
    expect(current.getAttribute('aria-pressed')).toBe('true');
    expect(document.querySelectorAll('.sc-model-version-selected')).toHaveLength(1);
    expect(accept.querySelector('.sc-model-action-icon')).toBeNull();
    current.click();
    await nextTick();
    expect(document.querySelectorAll('.sc-model-version-selected')).toHaveLength(0);
    expect(accept.disabled).toBe(true);
    expect(accept.querySelectorAll('.sc-model-action-icon')).toHaveLength(1);
    current.click();
    await nextTick();
    const icon = document.querySelector<HTMLImageElement>('.sc-model-family-footer-meta a img');
    expect(icon?.src).toContain('/model-providers/civitai.svg');
    const timeline = document.querySelector('.sc-model-timeline');
    const footer = document.querySelector('.sc-model-version-modal .p-dialog-footer');
    const content = document.querySelector('.sc-model-version-modal .p-dialog-content');
    if (!timeline || !footer || !content) throw new Error('Missing compact modal regions.');
    expect(getComputedStyle(timeline).height).toBe('auto');
    expect(getComputedStyle(content).paddingBottom).toBe('0px');
    expect(getComputedStyle(footer).paddingTop).toBe('8px');
    const check = current.querySelector('.sc-model-version-check');
    if (!check) throw new Error('Missing selected checkbox.');
    expect(getComputedStyle(check).display).toBe('grid');
    expect(getComputedStyle(check).placeItems).toBe('center');
    const mark = check.querySelector('svg');
    if (!mark) throw new Error('Missing centered checkmark.');
    expect(mark.getAttribute('viewBox')).toBe('0 0 16 16');
    expect(getComputedStyle(mark).width).toBe('14px');
    expect(getComputedStyle(mark).height).toBe('14px');
    expect(oldest.querySelector('.sc-model-version-check svg')).toBeNull();
    const rules = Array.from(style.sheet?.cssRules ?? []);
    const selectionRule = rules.find(
      (rule) => rule instanceof CSSStyleRule && rule.selectorText === '.sc-model-version-selected',
    );
    const checkRule = rules.find(
      (rule) =>
        rule instanceof CSSStyleRule &&
        rule.selectorText === '.sc-model-version-selected .sc-model-version-check',
    );
    if (!(selectionRule instanceof CSSStyleRule) || !(checkRule instanceof CSSStyleRule))
      throw new Error('Missing accent styling.');
    expect(selectionRule.style.getPropertyValue('border-color')).toBe(
      'var(--sc-brand-fill,#0078d4)',
    );
    expect(selectionRule.style.getPropertyValue('box-shadow')).toContain(
      'var(--sc-brand-fill,#0078d4)',
    );
    expect(checkRule.style.getPropertyValue('background')).toBe('var(--sc-brand-fill,#0078d4)');
    expect(checkRule.style.getPropertyValue('border-color')).toBe('var(--sc-brand-fill,#0078d4)');
    expect(checkRule.style.getPropertyValue('color')).toBe('var(--sc-text-on-accent,#fff)');
    accept.click();
    await closed;
    expect(select).toHaveBeenCalledWith(item.value);
    expect(acquire).not.toHaveBeenCalled();
  } finally {
    dialog.close();
    await closed;
    style.remove();
  }
});

test.each([true, false])(
  'accepts a version with onDisk=%s and selects its exact host filename',
  async (onDisk) => {
    let value = 'original.safetensors';
    let options = [value, 'family/renamed.safetensors'];
    const anchor = document.createElement('div');
    document.body.append(anchor);
    const session = new ModelPickerSession(document, () => ({
      label: 'model',
      kind: 'checkpoints',
      options,
      anchor,
      readValue: () => value,
      writeValue: (next) => {
        value = next;
      },
      disabled: () => false,
      subscribe: () => () => undefined,
    }));
    const item = modelPickerChoices([], 'checkpoints', [value])[0];
    if (!item) throw new Error('Missing model.');
    const release: ModelRelease = {
      versionId: '21',
      name: '<b>New version</b>',
      baseModel: 'SDXL',
      publishedAt: '',
      pageUrl: '',
      previewUrl: null,
      onDisk,
      inUse: false,
      installedFileName: onDisk ? 'family/renamed.safetensors' : null,
      files: [
        {
          access: 'public',
          fileId: '21',
          fileName: 'provider-name.safetensors',
          sha256: 'b'.repeat(64),
          sizeBytes: 10,
          kind: 'checkpoints',
        },
      ],
    };
    const acquire = jest.fn<ModelPickerActions['acquire']>(
      async (_model, _version, _file, _status, _signal, transfer) => {
        expect(value).toBe('original.safetensors');
        transfer?.({ message: 'Downloading…', percentage: 25 });
        expect(session.read()?.percentage).toBe(25);
        expect(
          document.querySelectorAll(
            '.sc-model-version-modal .sugarcubes-modal__footer-actions button:last-child .sc-model-action-icon',
          ),
        ).toHaveLength(1);
        options = ['original.safetensors', 'family\\renamed.safetensors'];
        return 'family/renamed.safetensors';
      },
    );
    const dialog = new ModelVersionDialog(
      document,
      item,
      {
        credentials: {
          status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
          save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
        },
        classifyContent: async () => undefined,
        refreshMetadata: async () => undefined,
        replaceThumbnail: async () => undefined,
        releases: async () => [release],
        acquire,
      },
      session,
      () => undefined,
    );
    const closed = dialog.open();
    try {
      await settle();
      const card = document.querySelector<HTMLButtonElement>('.sc-model-version-card');
      const accept = document.querySelector<HTMLButtonElement>(
        '.sc-model-version-modal .sugarcubes-modal__footer-actions button:last-child',
      );
      if (!card || !accept) throw new Error('Missing version controls.');
      expect(card.disabled).toBe(false);
      card.click();
      await nextTick();
      expect(accept.textContent).toBe(onDisk ? 'Use version' : 'Download and use');
      expect(accept.querySelectorAll('.sc-model-action-icon')).toHaveLength(onDisk ? 0 : 1);
      expect(value).toBe('original.safetensors');
      card.click();
      await nextTick();
      expect(accept.disabled).toBe(true);
      card.click();
      await nextTick();
      accept.click();
      await closed;
      expect(acquire).toHaveBeenCalledTimes(onDisk ? 0 : 1);
      expect(value.replaceAll('\\', '/')).toBe('family/renamed.safetensors');
      expect(session.read()).toBeNull();
      expect(document.querySelector('[role=dialog]')).toBeNull();
    } finally {
      dialog.close();
      session.dispose();
      anchor.remove();
    }
  },
);
