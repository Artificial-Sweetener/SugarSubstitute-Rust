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
/** Prove owned saving controls, stable drafts and modal recovery without changing graph or browser preferences. */
import { beforeEach, jest } from '@jest/globals';
import type { CubeLibraryApi } from '../../../frontend/comfyui/ui/core/CubeLibraryApi.js';
import { OutputSavingClient } from '../../../frontend/comfyui/ui/output_saving/OutputSavingClient.js';
import { OutputSavingDialog } from '../../../frontend/comfyui/ui/output_saving/OutputSavingDialog.js';
import { OutputSavingForm } from '../../../frontend/comfyui/ui/output_saving/OutputSavingForm.js';
import { pickSubstituteSettings } from '../../../frontend/comfyui/ui/output_saving/SubstituteSettingsPicker.js';
import {
  savingPreferences,
  savingResponse,
  savingSnapshot,
} from '../support/SavingSettingsFixture.js';

const adapter = { getDocument: () => document, getWindow: () => window };

/** Require real accessible dialog controls rather than inspecting private renderer state. */
function control<T extends HTMLElement>(label: string, type: new (...args: never[]) => T): T {
  const element = document.querySelector(`[aria-label="${label}"]`);
  if (!(element instanceof type)) throw new Error('Missing settings control: ' + label);
  return element;
}

/** Settle asynchronous transport and modal transactions at their public boundaries. */
async function settle(): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
}

beforeEach(() => {
  document.body.replaceChildren();
  localStorage.clear();
});

test('retains inactive JPEG and workflow choices without losing control identity or parsing output text', () => {
  const form = new OutputSavingForm(document, savingSnapshot());
  document.body.append(form.root);
  expect(form.read()).toEqual(savingPreferences);
  expect(form.previews.disabled).toBe(true);
  expect(form.quality.disabled).toBe(false);
  expect(form.transfer.value).toBe('companion_jpeg');
  form.jpeg.click();
  form.management.click();
  expect(form.previews.disabled).toBe(false);
  expect(form.quality.disabled).toBe(false);
  form.jpegMode.value = 'target_size';
  form.jpegMode.dispatchEvent(new Event('change', { bubbles: true }));
  expect(form.quality.disabled).toBe(false);
  expect(form.targetSize.disabled).toBe(false);
  form.jpeg.click();
  form.management.click();
  expect(form.read().jpeg.quality).toBe(92);
  expect(form.read().jpeg.target_size_kib).toBe(800);
  expect(form.read().save_previews).toBe(true);
  expect(form.root.querySelector('script')).toBeNull();
  expect(form.example.textContent).toBe(savingSnapshot().examplePath);
});

test('failed save preserves the draft for retry and cancellation never writes browser settings', async () => {
  const fetchJson = jest
    .fn<CubeLibraryApi['fetchJson']>()
    .mockResolvedValueOnce({
      response: { ok: false },
      data: { success: false, error: { message: 'Folder is unavailable' } },
    })
    .mockResolvedValue(savingResponse());
  const dialog = new OutputSavingDialog(document, new OutputSavingClient({ fetchJson }), adapter);
  const result = dialog.open(savingSnapshot());
  const pattern = control('Output pattern', HTMLInputElement);
  pattern.value = '{run}_{source}';
  const confirm = document.querySelector('.sugarcubes-modal__footer-actions button:last-child');
  if (!(confirm instanceof HTMLButtonElement)) throw new Error('Missing saving confirmation.');
  confirm.click();
  await settle();
  expect(document.querySelector('.sugarcubes-modal__error')?.textContent).toBe(
    'Folder is unavailable',
  );
  expect(control('Output pattern', HTMLInputElement)).toBe(pattern);
  expect(pattern.value).toBe('{run}_{source}');
  expect(confirm.disabled).toBe(false);
  confirm.click();
  await expect(result).resolves.toEqual(savingSnapshot());
  expect(fetchJson).toHaveBeenCalledTimes(2);
  expect(localStorage.length).toBe(0);
  const cancelled = dialog.open(savingSnapshot());
  control('Close dialog', HTMLButtonElement).click();
  await expect(cancelled).resolves.toBeNull();
  expect(fetchJson).toHaveBeenCalledTimes(2);
});

test('typing requests one bounded nonmutating example and cancelling aborts the request', async () => {
  jest.useFakeTimers();
  try {
    const fetchJson = jest.fn<CubeLibraryApi['fetchJson']>().mockResolvedValue(savingResponse());
    const result = new OutputSavingDialog(
      document,
      new OutputSavingClient({ fetchJson }),
      adapter,
    ).open(savingSnapshot());
    const pattern = control('Output pattern', HTMLInputElement);
    pattern.value = '{source}';
    pattern.dispatchEvent(new Event('input', { bubbles: true }));
    await jest.advanceTimersByTimeAsync(250);
    expect(fetchJson.mock.calls[0]?.[0]).toBe('/sugarcubes/output-saving/preview');
    expect(fetchJson.mock.calls[0]?.[1]?.body).toContain('"path_pattern":"{source}"');
    control('Close dialog', HTMLButtonElement).click();
    await expect(result).resolves.toBeNull();
    expect(jest.getTimerCount()).toBe(0);
  } finally {
    jest.useRealTimers();
  }
});

test('explicit import picker cancellation releases its owned element and never reads a file', async () => {
  const result = pickSubstituteSettings(document);
  const input = document.querySelector('input[type="file"]');
  if (!(input instanceof HTMLInputElement)) throw new Error('Missing source settings picker.');
  input.dispatchEvent(new Event('cancel'));
  await expect(result).resolves.toBeNull();
  expect(input.isConnected).toBe(false);
});
