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
/** Prove the separate version modal preserves native selection and requires a deliberate compatible download. */
import { nextTick } from 'vue';
import { jest } from '@jest/globals';
import { ModelVersionDialog } from '../../../frontend/comfyui/ui/model_library/ModelVersionDialog.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import type { ModelPickerActions } from '../../../frontend/comfyui/ui/model_library/ModelPickerActions.js';
import type { ModelRelease } from '../../../frontend/comfyui/ui/model_library/ModelReleases.js';
import { modelPickerCss } from '../../../frontend/comfyui/ui/model_library/ModelPickerStyles.js';
import { ModelUpdateLifecycle } from '../../../frontend/comfyui/ui/model_library/ModelUpdateLifecycle.js';
import type { ModelUpdatePort } from '../../../frontend/comfyui/ui/model_library/ModelUpdates.js';
import { compatibleModel } from '../support/model_compatibility.js';

test.each([
  ['Illustrious', 'Illustrious'],
  ['SDXL', 'Base Model'],
])('presents %s family without redundant architecture text', async (familyKey, familyLabel) => {
  const base = compatibleModel('installed.safetensors', 'checkpoints', 'SDXL', familyKey ?? '');
  if (!base.traits) throw new Error('Missing prepared family.');
  const item = {
    ...base,
    traits: {
      ...base.traits,
      familyName: familyKey === 'SDXL' ? 'Stable Diffusion XL' : (familyKey ?? ''),
    },
    title: 'Amanatsu <img> (Illustrious)',
  };
  const dialog = versionDialog(document, item, {
    credentials: {
      status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
    },
    classifyContent: async () => undefined,
    refreshMetadata: async () => undefined,
    replaceThumbnail: async () => undefined,
    releases: async () => [release('20'), release('21')],
    acquire: async () => '21.safetensors',
  });
  const closed = dialog.open();
  try {
    await settle();
    const surface = document.querySelector('[role=dialog]');
    const title = `Versions of Amanatsu <img> (Illustrious) - SDXL \\ ${familyLabel}`;
    expect(surface?.querySelector('.p-dialog-title')?.textContent).toBe(title);
    expect(surface?.getAttribute('aria-label')).toBe(title);
    expect(surface?.querySelector('.p-dialog-title img')).toBeNull();
    expect(surface?.querySelector('.sc-model-family-summary')).toBeNull();
    expect(surface?.querySelector('.sc-model-version-status')).toBeNull();
    expect(surface?.textContent).not.toContain('newest to oldest');
    expect(surface?.textContent).not.toContain('Checkpoint ·');
  } finally {
    dialog.close();
    await closed;
  }
});

test('does not start provider checks on focus during or after version browsing', async () => {
  const item = modelPickerChoices([], 'loras', ['installed.safetensors'])[0];
  if (!item) throw new Error('Missing model.');
  const check = jest.fn<ModelUpdatePort['check']>(async () => undefined);
  const updates: ModelUpdatePort = {
    read: () => ({ enabled: true, proposals: [] }),
    subscribe: () => () => undefined,
    load: async () => undefined,
    check,
    checkSelection: async () => undefined,
    setEnabled: async () => undefined,
    dismiss: async () => undefined,
    disableModel: async () => undefined,
  };
  const focus = new ModelUpdateLifecycle(window, updates, jest.fn());
  const dialog = versionDialog(document, item, {
    credentials: {
      status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
    },
    classifyContent: async () => undefined,
    refreshMetadata: async () => undefined,
    replaceThumbnail: async () => undefined,
    releases: async () => [release('20'), release('21')],
    acquire: async () => '21.safetensors',
  });
  focus.start();
  await settle();
  const baseline = check.mock.calls.length;
  expect(baseline).toBe(0);
  const closed = dialog.open();
  try {
    await settle();
    window.dispatchEvent(new Event('focus'));
    await settle();
    expect(check).toHaveBeenCalledTimes(baseline);
    dialog.close();
    await closed;
    window.dispatchEvent(new Event('focus'));
    await settle();
    expect(check).toHaveBeenCalledTimes(baseline);
  } finally {
    focus.dispose();
    dialog.close();
    await closed;
  }
});

test('shows loading in the empty timeline and keeps transfer feedback out of the modal', async () => {
  const item = modelPickerChoices([], 'loras', ['installed.safetensors'])[0];
  if (!item) throw new Error('Missing model.');
  let finishLookup: (versions: readonly ModelRelease[]) => void = () => {
    throw new Error('Family lookup not started.');
  };
  let finishTransfer: (filename: string) => void = () => {
    throw new Error('Transfer not started.');
  };
  const dialog = versionDialog(document, item, {
    credentials: {
      status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
    },
    classifyContent: async () => undefined,
    refreshMetadata: async () => undefined,
    replaceThumbnail: async () => undefined,
    releases: () =>
      new Promise((resolve) => {
        finishLookup = resolve;
      }),
    acquire: () =>
      new Promise((resolve) => {
        finishTransfer = resolve;
      }),
  });
  const closed = dialog.open();
  try {
    await nextTick();
    const summary = document.querySelector('.sc-model-version-loading');
    expect(summary?.querySelector('.sc-model-busy-ring')?.getAttribute('viewBox')).toBe(
      '0 0 22 22',
    );
    expect(summary?.querySelector('.sc-model-busy-ring circle')?.getAttribute('r')).toBe('8');
    expect(summary?.querySelector('.sc-model-busy-ring circle')?.getAttribute('stroke-width')).toBe(
      '6',
    );
    finishLookup([release('20'), release('21')]);
    await settle();
    expect(document.querySelector('.sc-model-version-status')).toBeNull();
    document
      .querySelector<HTMLButtonElement>('.sc-model-version-card:not([data-in-use=true])')
      ?.click();
    await nextTick();
    const download = document.querySelector<HTMLButtonElement>(
      '.sc-model-version-modal .sugarcubes-modal__footer-actions button:last-child',
    );
    if (!download) throw new Error('Missing download action.');
    download.click();
    await nextTick();
    expect(document.querySelector('.sc-model-version-status')).toBeNull();
    finishTransfer('21.safetensors');
    await settle();
    expect(document.querySelector('[role=dialog]')).toBeNull();
    expect(item.value).toBe('installed.safetensors');
  } finally {
    dialog.close();
    await closed;
  }
});

test('settles version preview loading and failure without changing selection or acquisition', async () => {
  const item = modelPickerChoices([], 'loras', ['installed.safetensors'])[0];
  if (!item) throw new Error('Missing model.');
  const acquire = jest.fn<ModelPickerActions['acquire']>(async () => '21.safetensors');
  const dialog = versionDialog(document, item, {
    credentials: {
      status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
    },
    classifyContent: async () => undefined,
    refreshMetadata: async () => undefined,
    replaceThumbnail: async () => undefined,
    releases: async () => [release('20'), { ...release('21'), previewUrl: '/preview-21' }],
    acquire,
  });
  const closed = dialog.open();
  try {
    await settle();
    const cards = document.querySelectorAll<HTMLButtonElement>('.sc-model-version-card');
    const available = cards[0];
    if (!available) throw new Error('Missing available version.');
    expect(cards[1]?.textContent).toContain('Preview unavailable');
    expect(available.querySelector('[role=status]')?.textContent).toContain('Loading preview');
    available.click();
    await settle();
    expect(available.getAttribute('aria-pressed')).toBe('true');
    available.querySelector('img')?.dispatchEvent(new Event('error'));
    await settle();
    expect(available.querySelector('img')).toBeNull();
    expect(available.textContent).toContain('Preview unavailable');
    expect(available.querySelector('[role=status]')).toBeNull();
    expect(available.getAttribute('aria-pressed')).toBe('true');
    expect(item.value).toBe('installed.safetensors');
    expect(acquire).not.toHaveBeenCalled();
  } finally {
    dialog.close();
    await closed;
  }
});

/** Drain provider promises before observing the mounted Vue card state. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  await nextTick();
}

/** Create already-validated ordered offers rather than giving the dialog provider URLs or paths. */
function release(versionId: string, onDisk = false): ModelRelease {
  return {
    versionId,
    name: `<b>${versionId}</b>`,
    baseModel: 'SDXL',
    publishedAt: '',
    pageUrl: 'https://civitai.com/models/10',
    previewUrl: null,
    onDisk,
    installedFileName: onDisk ? `${versionId}.safetensors` : null,
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

test('uses server current-version evidence and navigation when catalog provider metadata is absent', async () => {
  const item = modelPickerChoices([], 'loras', ['installed.safetensors'])[0];
  if (!item) throw new Error('Missing model.');
  const dialog = versionDialog(document, item, {
    credentials: {
      status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
    },
    classifyContent: async () => undefined,
    refreshMetadata: async () => undefined,
    replaceThumbnail: async () => undefined,
    releases: async () => [release('20'), release('21')],
    acquire: async () => '21.safetensors',
  });
  const closed = dialog.open();
  try {
    await settle();
    const cards = [...document.querySelectorAll<HTMLButtonElement>('.sc-model-version-card')];
    expect(cards[1]?.textContent).toContain('On disk');
    expect(cards[1]?.getAttribute('data-in-use')).toBe('true');
    expect(cards[0]?.disabled).toBe(false);
    expect(cards[1]?.disabled).toBe(false);
    expect(document.querySelector('.p-dialog-title')?.textContent).toBe(
      `Versions of ${item.title} - SDXL`,
    );
    expect(document.querySelector('.sc-model-family-footer-meta a')?.getAttribute('href')).toBe(
      'https://civitai.com/models/10',
    );
    expect(item.providerVersionId).toBe('');
    expect(item.value).toBe('installed.safetensors');
  } finally {
    dialog.close();
    await closed;
  }
});

test('shows On disk for every installed version and never downloads on initial render', async () => {
  const base = modelPickerChoices([], 'loras', ['installed.safetensors'])[0];
  if (!base) throw new Error('Missing model.');
  const item = {
    ...base,
    id: 'a'.repeat(64),
    title: '<img>Fixture',
    provider: 'civitai',
    baseModel: 'SDXL',
    providerVersionId: '20',
    providerUrl: 'https://civitai.com/models/10',
  };
  const acquire = jest.fn<ModelPickerActions['acquire']>(async () => '21.safetensors');
  const actions: ModelPickerActions = {
    credentials: {
      status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
    },
    classifyContent: async () => undefined,
    refreshMetadata: async () => undefined,
    replaceThumbnail: async () => undefined,
    releases: async () => [release('19', true), release('20'), release('21')],
    acquire,
  };
  const dialog = versionDialog(document, item, actions);
  const closed = dialog.open();
  await settle();
  const style = document.createElement('style');
  style.textContent = modelPickerCss;
  document.head.append(style);
  try {
    const surface = document.querySelector('[role=dialog]');
    expect(surface?.getAttribute('aria-label')).toBe('Versions of <img>Fixture - SDXL');
    expect(surface?.querySelector('.sc-model-version-name img')).toBeNull();
    expect(surface?.querySelector('b')).toBeNull();
    const provider = surface?.querySelector<HTMLElement>('.sc-model-family-footer-meta a');
    if (!provider) throw new Error('Missing provider navigation.');
    expect(surface?.querySelector('.sc-model-family-summary')).toBeNull();
    expect(surface?.querySelector('.sc-model-version-status')).toBeNull();
    expect(getComputedStyle(provider).height).toBe('32px');
    const cards = [...document.querySelectorAll<HTMLButtonElement>('.sc-model-version-card')];
    expect(cards.map((card) => card.textContent)).toEqual([
      'Preview unavailable<b>21</b>10 B',
      'Preview unavailable<b>20</b>On disk',
      'Preview unavailable<b>19</b>On disk',
    ]);
    expect(cards.map((card) => card.disabled)).toEqual([false, false, false]);
    expect(acquire).not.toHaveBeenCalled();
    const download = document.querySelector<HTMLButtonElement>(
      '.sc-model-version-modal .sugarcubes-modal__footer-actions button:last-child',
    );
    if (!download) throw new Error('Missing download button.');
    expect(download.disabled).toBe(false);

    expect(surface?.querySelector('a img.sc-model-provider-icon')?.getAttribute('src')).toContain(
      'civitai.svg',
    );
    const cancelTransfer = surface?.querySelector('.sc-model-family-footer-meta button');
    expect(cancelTransfer).toBeNull();
    cards[0]?.click();
    await nextTick();
    expect(download.disabled).toBe(false);
    download.click();
    await settle();
    expect(acquire.mock.calls[0]?.slice(0, 3)).toEqual([item.id, '21', '21']);
    expect(document.querySelector('[role=dialog]')).toBeNull();
    expect(item.value).toBe('installed.safetensors');
  } finally {
    dialog.close();
    await closed;
    style.remove();
  }
  expect(document.querySelector('[role=dialog]')).toBeNull();
});

test('shows literal provider failures and cancels pending history when its modal closes', async () => {
  const base = modelPickerChoices([], 'loras', ['installed.safetensors'])[0];
  if (!base) throw new Error('Missing model.');
  let signal: AbortSignal | null = null;
  let fail = true;
  const actions: ModelPickerActions = {
    credentials: {
      status: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
      save: async () => ({ configured: true, storageAvailable: true, storageReason: null }),
    },
    classifyContent: async () => undefined,
    refreshMetadata: async () => undefined,
    replaceThumbnail: async () => undefined,
    acquire: async () => '21.safetensors',
    releases: async (_id, current) => {
      if (fail) throw new Error('<b>Provider unavailable.</b>');
      signal = current;
      await new Promise<void>((_resolve, reject) => {
        current.addEventListener('abort', () => reject(new Error('Cancelled.')), { once: true });
      });
      return [];
    },
  };
  const logging = jest.spyOn(console, 'error').mockImplementation(() => undefined);
  try {
    const feedback = jest.fn();
    const first = versionDialog(document, base, actions, feedback);
    const firstClosed = first.open();
    await settle();
    expect(feedback).toHaveBeenCalledWith('error', '<b>Provider unavailable.</b>');
    expect(document.querySelector('.sugarcubes-modal__error')?.textContent).toBe('');
    expect(document.querySelector('[role=alert] b')).toBeNull();
    expect(document.body.textContent).not.toContain('Loading model versions');
    first.close();
    await firstClosed;
    fail = false;
    const second = versionDialog(document, base, actions);
    const secondClosed = second.open();
    await settle();
    second.close();
    await secondClosed;
    const observed: unknown = signal;
    if (!(observed instanceof AbortSignal)) throw new Error('No history signal.');
    expect(observed.aborted).toBe(true);
    expect(document.querySelector('[role=dialog]')).toBeNull();
  } finally {
    logging.mockRestore();
  }
});

/** Bind an explicit receiver for dialog-only rendering and acquisition proofs. */
function versionDialog(
  document: Document,
  item: import('../../../frontend/comfyui/ui/model_library/ModelCatalog.js').ModelCatalogItem,
  actions: import('../../../frontend/comfyui/ui/model_library/ModelPickerActions.js').ModelPickerActions,
  feedback: import('../../../frontend/comfyui/ui/model_library/ModelPickerActions.js').ModelPickerFeedback = () =>
    undefined,
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
    feedback,
  );
}
