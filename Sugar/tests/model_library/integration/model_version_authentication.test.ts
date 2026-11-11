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
/** Prove protected version selection and recoverable transfers keep the version layout stable. */
import { jest } from '@jest/globals';
import { nextTick } from 'vue';
import { ModelVersionDialog } from '../../../frontend/comfyui/ui/model_library/ModelVersionDialog.js';
import { ModelPickerSession } from '../../../frontend/comfyui/ui/model_library/ModelPickerSession.js';
import { modelPickerChoices } from '../../../frontend/comfyui/ui/model_library/ModelPickerChoices.js';
import type { ModelPickerActions } from '../../../frontend/comfyui/ui/model_library/ModelPickerActions.js';
import type { ModelRelease } from '../../../frontend/comfyui/ui/model_library/ModelReleases.js';

/** Drain explicit credential, provider and Vue transitions. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  await nextTick();
}
/** Find the stable accept slot without relying on model names. */
function accept(): HTMLButtonElement {
  const button = document.querySelector<HTMLButtonElement>(
    '.sc-model-version-modal .sugarcubes-modal__footer-actions button:last-child',
  );
  if (!button) throw new Error('Missing accept action.');
  return button;
}
/** Supply one protected remote artifact or an already installed version. */
function release(onDisk = false): ModelRelease {
  return {
    versionId: '20',
    name: 'v2',
    baseModel: 'Illustrious',
    publishedAt: '',
    pageUrl: 'https://civitai.com/models/10',
    previewUrl: null,
    onDisk,
    installedFileName: onDisk ? 'installed.safetensors' : null,
    inUse: false,
    files: [
      {
        fileId: '30',
        fileName: 'v2.safetensors',
        sha256: 'a'.repeat(64),
        sizeBytes: 1024,
        kind: 'checkpoints',
        access: 'api_key_required',
      },
    ],
  };
}
/** Bind a legal native field and independently observable credential, transfer and feedback boundaries. */
function fixture(onDisk = false) {
  const item = modelPickerChoices([], 'checkpoints', ['installed.safetensors'])[0];
  if (!item) throw new Error('Missing field model.');
  let configured = false;
  const status = jest.fn(async (_signal: AbortSignal) => ({
    configured,
    storageAvailable: true,
    storageReason: null,
  }));
  const save = jest.fn(async (_key: string) => {
    configured = true;
    return { configured, storageAvailable: true, storageReason: null };
  });
  const acquire = jest.fn<ModelPickerActions['acquire']>();
  const feedback = jest.fn();
  const session = new ModelPickerSession(document, () => ({
    anchor: document.createElement('div'),
    label: 'Model',
    kind: 'checkpoints',
    options: ['installed.safetensors'],
    readValue: () => item.value,
    writeValue: () => undefined,
    disabled: () => false,
    subscribe: () => () => undefined,
  }));
  const actions: ModelPickerActions = {
    credentials: { status, save },
    classifyContent: async () => undefined,
    refreshMetadata: async () => undefined,
    replaceThumbnail: async () => undefined,
    releases: async () => [release(onDisk)],
    acquire,
  };
  const dialog = new ModelVersionDialog(document, item, actions, session, feedback);
  return { dialog, session, status, save, acquire, feedback };
}

test('keeps download size and button while showing a protected-card key badge and stacked key prompt', async () => {
  const proof = fixture();
  const closed = proof.dialog.open();
  try {
    await settle();
    const card = document.querySelector<HTMLButtonElement>('.sc-model-version-card');
    if (!card) throw new Error('Missing version.');
    expect(card.querySelector('.sc-model-version-state')?.textContent).toBe('1 KiB');
    expect(card.querySelector('[role=img][aria-label="CivitAI API key required"]')).not.toBeNull();
    expect(card.getAttribute('aria-label')).toBe('v2, API key required');
    card.click();
    await settle();
    expect(accept().textContent).toBe('Download and use');
    expect(accept().querySelector<HTMLElement>('.sc-model-action-icon')?.style.cssText).toContain(
      'download.svg',
    );
    accept().click();
    await settle();
    expect(proof.acquire).not.toHaveBeenCalled();
    expect(proof.session.read()).toBeNull();
    const prompt = document.querySelector('.sc-model-key-overlay');
    expect(prompt).not.toBeNull();
    expect(prompt?.textContent).toContain('This model requires a CivitAI API key to download.');
    expect(prompt?.querySelector('[role=dialog]')?.getAttribute('aria-modal')).toBe('true');
    expect(document.querySelector('.sc-model-version-modal')).toHaveProperty('inert', true);
    expect(prompt?.querySelector('input')?.type).toBe('password');
    prompt?.querySelector<HTMLButtonElement>('.sugarcubes-modal__footer-actions button')?.click();
    await settle();
    expect(proof.save).not.toHaveBeenCalled();
    expect(proof.acquire).not.toHaveBeenCalled();
    expect(card.getAttribute('aria-pressed')).toBe('true');
    expect(document.querySelector('.sc-model-version-modal')).toHaveProperty('inert', false);
    expect(accept().textContent).toBe('Download and use');
    accept().click();
    await settle();
    const secondPrompt = document.querySelector('.sc-model-key-overlay');
    secondPrompt
      ?.querySelector('input')
      ?.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
      );
    await settle();
    expect(document.querySelector('.sc-model-key-overlay')).toBeNull();
    expect(document.querySelector('.sc-model-version-modal')).not.toBeNull();
    expect(card.getAttribute('aria-pressed')).toBe('true');
    expect(proof.acquire).not.toHaveBeenCalled();
  } finally {
    proof.dialog.close();
    await closed;
    proof.session.dispose();
  }
});

test('cancels pending key-status work when the originating version dialog closes', async () => {
  const proof = fixture();
  const closed = proof.dialog.open();
  const logging = jest.spyOn(console, 'error').mockImplementation(() => undefined);
  let pendingSignal: AbortSignal | null = null;
  try {
    await settle();
    proof.status.mockImplementationOnce(
      (signal) =>
        new Promise((_resolve, reject) => {
          pendingSignal = signal;
          signal.addEventListener('abort', () => reject(new Error('Origin closed.')), {
            once: true,
          });
        }),
    );
    document.querySelector<HTMLButtonElement>('.sc-model-version-card')?.click();
    await settle();
    accept().click();
    await settle();
    proof.dialog.close();
    await closed;
    await settle();
    expect(pendingSignal).not.toBeNull();
    expect(pendingSignal).toHaveProperty('aborted', true);
    expect(proof.acquire).not.toHaveBeenCalled();
    expect(proof.feedback).not.toHaveBeenCalled();
    expect(logging).not.toHaveBeenCalled();
    expect(document.querySelector('.sc-model-key-overlay')).toBeNull();
  } finally {
    proof.dialog.close();
    await closed;
    proof.session.dispose();
    logging.mockRestore();
  }
});

test('saves explicit keys and routes progress to the combo and errors to feedback without adding modal rows', async () => {
  const proof = fixture();
  let rejectTransfer: (error: Error) => void = () => {
    throw new Error('No transfer.');
  };
  let cancelled = false;
  proof.acquire.mockImplementation(async (_model, _version, _file, _status, signal, transfer) => {
    transfer?.({ message: 'Downloading 25%', percentage: 25 });
    return new Promise<string>((_resolve, reject) => {
      rejectTransfer = reject;
      signal.addEventListener('abort', () => {
        cancelled = true;
        reject(new Error('Cancelled'));
      });
    });
  });
  const closed = proof.dialog.open();
  const logging = jest.spyOn(console, 'error').mockImplementation(() => undefined);
  try {
    await settle();
    document.querySelector<HTMLButtonElement>('.sc-model-version-card')?.click();
    await settle();
    accept().click();
    await settle();
    const prompt = document.querySelector('.sc-model-key-overlay');
    const input = prompt?.querySelector('input');
    if (!input) throw new Error('Missing key input.');
    input.value = 'explicit-test-key';
    input.dispatchEvent(new Event('input'));
    prompt
      ?.querySelector<HTMLButtonElement>('.sugarcubes-modal__footer-actions button:last-child')
      ?.click();
    await settle();
    expect(proof.save).toHaveBeenCalledWith('explicit-test-key', expect.any(AbortSignal));
    expect(proof.acquire).toHaveBeenCalledTimes(1);
    expect(proof.session.read()?.percentage).toBe(25);
    expect(document.querySelector('.sc-model-version-modal')).toBeNull();
    await closed;
    expect(cancelled).toBe(false);
    rejectTransfer(new Error('<b>Access rejected</b>'));
    await settle();
    expect(proof.feedback).toHaveBeenCalledWith('error', '<b>Access rejected</b>');
    expect(proof.session.read()).toBeNull();
    expect(document.querySelector('.sc-model-version-modal')).toBeNull();
    expect(cancelled).toBe(false);
  } finally {
    proof.dialog.close();
    await closed;
    proof.session.dispose();
    logging.mockRestore();
  }
});

test('keeps protected installed versions usable without API keys', async () => {
  const proof = fixture(true);
  const closed = proof.dialog.open();
  try {
    await settle();
    expect(accept().textContent).toBe('Use version');
    expect(document.querySelector('.sc-model-version-state')?.textContent).toBe('On disk');
    expect(document.querySelector('.sc-model-version-card .sc-model-key-required')).toBeNull();
    accept().click();
    await closed;
    expect(proof.acquire).not.toHaveBeenCalled();
    expect(proof.save).not.toHaveBeenCalled();
  } finally {
    proof.dialog.close();
    proof.session.dispose();
  }
});
