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
/** Safeguard version-card geometry independently of selected and preview states. */
import { nextTick } from 'vue';
import { ModelVersionDialog } from '../../../frontend/comfyui/ui/model_library/ModelVersionDialog.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import { modelPickerCss } from '../../../frontend/comfyui/ui/model_library/ModelPickerStyles.js';
import type { ModelRelease } from '../../../frontend/comfyui/ui/model_library/ModelReleases.js';

test('keeps card and portrait geometry unchanged through exclusive selection and preview settlement', async () => {
  const item = modelPickerChoices([], 'loras', ['installed.safetensors'])[0];
  if (!item) throw new Error('Missing installed model.');
  const dialog = versionDialog(document, item, {
    credentials: {
      status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
    },
    classifyContent: async () => undefined,
    refreshMetadata: async () => undefined,
    replaceThumbnail: async () => undefined,
    releases: async () => [version('20'), version('21'), version('22')],
    acquire: async () => '21.safetensors',
  });
  const style = document.createElement('style');
  style.textContent = modelPickerCss;
  document.head.append(style);
  const closed = dialog.open();
  try {
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    await nextTick();
    const cards = [...document.querySelectorAll<HTMLButtonElement>('.sc-model-version-card')];
    const first = cards[0];
    const second = cards[1];
    if (!first || !second) throw new Error('Missing available versions.');
    const before = cards.map(geometry);
    first.click();
    await nextTick();
    expect(first.getAttribute('aria-pressed')).toBe('true');
    expect(cards.map(geometry)).toEqual(before);
    first.querySelector('img')?.dispatchEvent(new Event('load'));
    await nextTick();
    expect(cards.map(geometry)).toEqual(before);
    second.click();
    await nextTick();
    expect(first.getAttribute('aria-pressed')).toBe('false');
    expect(second.getAttribute('aria-pressed')).toBe('true');
    expect(cards.map(geometry)).toEqual(before);
    expect(getComputedStyle(first).boxShadow).not.toBe(getComputedStyle(second).boxShadow);
    const download = document.querySelector<HTMLButtonElement>(
      '.sc-model-version-modal .sugarcubes-modal__footer-actions button:last-child',
    );
    if (!download) throw new Error('Missing download action.');
    expect(download.disabled).toBe(false);
    second.click();
    await nextTick();
    expect(cards.every((card) => card.getAttribute('aria-pressed') === 'false')).toBe(true);
    expect(download.disabled).toBe(true);
    expect(cards.map(geometry)).toEqual(before);
    first.click();
    await nextTick();
    expect(first.getAttribute('aria-pressed')).toBe('true');
    expect(download.disabled).toBe(false);
  } finally {
    dialog.close();
    await closed;
    style.remove();
  }
});

/** Capture properties that participate in the card's layout rather than its paint. */
function geometry(card: HTMLButtonElement): readonly string[] {
  const portrait = card.querySelector('.sc-model-version-portrait');
  if (!portrait) throw new Error('Missing portrait.');
  const frame = getComputedStyle(card);
  const image = getComputedStyle(portrait);
  return [
    frame.width,
    frame.height,
    frame.padding,
    frame.borderWidth,
    image.width,
    image.height,
    image.flexShrink,
  ];
}

/** Supply validated version offers without triggering an artifact download. */
function version(versionId: string): ModelRelease {
  return {
    versionId,
    name: versionId,
    baseModel: 'SDXL',
    publishedAt: '',
    pageUrl: 'https://civitai.com/models/10',
    previewUrl: '/preview',
    onDisk: false,
    inUse: versionId === '20',
    files: [
      {
        access: 'public',
        fileId: versionId,
        fileName: `${versionId}.safetensors`,
        sha256: 'b'.repeat(64),
        sizeBytes: 10,
        kind: 'loras',
      },
    ],
  };
}

/** Bind an explicit receiver for dialog-only rendering and acquisition proofs. */
function versionDialog(
  document: Document,
  item: import('../../../frontend/comfyui/ui/model_library/ModelCatalog.js').ModelCatalogItem,
  actions: import('../../../frontend/comfyui/ui/model_library/ModelPickerActions.js').ModelPickerActions,
): ModelVersionDialog {
  return new ModelVersionDialog(
    document,
    item,
    actions,
    {
      current: () => item.value,
      select: () => undefined,
      download: (operation, _model, signal) =>
        operation(() => undefined, signal ?? new AbortController().signal),
      useDownload: async (operation) => {
        await operation(() => undefined, new AbortController().signal);
      },
    },
    () => undefined,
  );
}
