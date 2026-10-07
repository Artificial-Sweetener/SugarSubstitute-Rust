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
/** Prove every shared modal receives one Fluent presentation without changing form ownership. */
import { afterEach, expect, test } from '@jest/globals';
import { ModalShell } from '../../../frontend/comfyui/ui/dialogs/ModalShell.js';
import { InputModal } from '../../../frontend/comfyui/ui/dialogs/InputModal.js';
import { FormModal } from '../../../frontend/comfyui/ui/dialogs/FormModal.js';
import { SaveNodePresetDialog } from '../../../frontend/comfyui/ui/editor_actions/presentation/SaveNodePresetDialog.js';
import { GLOBAL_ASSOCIATION } from '../../../frontend/comfyui/ui/editor_actions/domain/UserPreset.js';

const adapter = { getDocument: () => document, getWindow: () => window };

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
  document.documentElement.style.removeProperty('color-scheme');
});

test('themes existing and purpose-built forms through the shared shell and retains native values', async () => {
  const input = new InputModal({ adapter });
  const inputResult = input.open({ title: '<b>Name</b>', initialValue: 'existing' });
  const form = new FormModal({ adapter });
  const formResult = form.open({
    fields: [{ key: 'enabled', type: 'checkbox', initialValue: true }],
  });
  const preset = new SaveNodePresetDialog(document);
  const presetResult = preset.open('Save preset', [
    GLOBAL_ASSOCIATION,
    {
      scope: 'model_family',
      provider: 'civitai',
      key: 'sdxl',
      label: 'SDXL',
    },
  ]);
  const overlays = [...document.querySelectorAll<HTMLElement>('.sugarcubes-modal-overlay')];
  expect(overlays).toHaveLength(3);
  expect(overlays.every((overlay) => overlay.dataset.scModalAppearance === 'fluent')).toBe(true);
  expect(document.querySelectorAll('#sugarcubes-modal-appearance')).toHaveLength(1);
  expect(document.querySelector('.sugarcubes-input-dialog input')).toHaveProperty(
    'value',
    'existing',
  );
  expect(document.querySelector('.sugarcubes-form-dialog input')).toHaveProperty('checked', true);
  expect(document.querySelectorAll('.sc-save-node-preset select option')).toHaveLength(2);
  expect(document.querySelector('.sugarcubes-input-dialog .p-dialog-title')?.textContent).toBe(
    '<b>Name</b>',
  );
  expect(document.querySelector('.sugarcubes-input-dialog b')).toBeNull();
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', cancelable: true }));
  await expect(presetResult).resolves.toBeNull();
  expect(overlays[0]?.classList.contains('is-visible')).toBe(true);
  expect(overlays[1]?.classList.contains('is-visible')).toBe(true);
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', cancelable: true }));
  await expect(formResult).resolves.toBeNull();
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', cancelable: true }));
  await expect(inputResult).resolves.toBeNull();
  expect(overlays.every((overlay) => !overlay.hasAttribute('data-sc-modal-appearance'))).toBe(true);
});

test('follows palette changes while open and releases the theme observer on close', async () => {
  document.documentElement.style.colorScheme = 'dark';
  const shell = new ModalShell({ adapter });
  const result = shell.open({ title: 'Shared', cancelResult: null });
  const overlay = shell.elements.overlay;
  const dark = overlay?.style.getPropertyValue('--colorNeutralBackground1');
  expect(dark).toBeTruthy();
  document.documentElement.style.colorScheme = 'light';
  await Promise.resolve();
  expect(overlay?.style.getPropertyValue('--colorNeutralBackground1')).not.toBe(dark);
  shell.close(null);
  await expect(result).resolves.toBeNull();
  expect(overlay?.style.getPropertyValue('--colorNeutralBackground1')).toBe('');
  document.documentElement.style.colorScheme = 'dark';
  await Promise.resolve();
  expect(overlay?.style.getPropertyValue('--colorNeutralBackground1')).toBe('');
});
