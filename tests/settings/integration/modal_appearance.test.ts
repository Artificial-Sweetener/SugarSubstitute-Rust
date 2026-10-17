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
/** Prove shared modal providers preserve form identity, validation, and native value ownership. */
import { afterEach, expect, test } from '@jest/globals';
import { ModalShell } from '../../../frontend/comfyui/ui/dialogs/ModalShell.js';
import { InputModal } from '../../../frontend/comfyui/ui/dialogs/InputModal.js';
import { FormModal } from '../../../frontend/comfyui/ui/dialogs/FormModal.js';
import { SaveNodePresetDialog } from '../../../frontend/comfyui/ui/editor_actions/presentation/SaveNodePresetDialog.js';
import { GLOBAL_ASSOCIATION } from '../../../frontend/comfyui/ui/editor_actions/domain/UserPreset.js';
import { AppearanceState } from '../../../frontend/comfyui/ui/surface/appearance/AppearanceState.js';
import { publishVisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyleBoundary.js';

const adapter = { getDocument: () => document, getWindow: () => window };

test('follows an accepted dual-style appearance source without replacing controls or retaining subscriptions after close', async () => {
  const source = new AppearanceState();
  source.update('fluent', true);
  const shell = new ModalShell({ adapter, appearanceSource: source });
  const input = document.createElement('input');
  input.value = 'retained';
  const result = shell.open({ title: 'Cube cart', body: input });
  input.focus();
  const overlay = shell.elements.overlay!;
  expect(overlay.dataset.scModalAppearance).toBe('fluent');
  source.update('comfy', true);
  expect(overlay.dataset.scModalAppearance).toBe('comfy');
  expect(overlay.style.getPropertyValue('--sc-surface')).toBe('');
  expect(document.activeElement).toBe(input);
  expect(input.value).toBe('retained');
  source.update('fluent', false);
  expect(overlay.dataset.scModalColorScheme).toBe('light');
  expect(overlay.dataset.scModalAppearance).toBe('fluent');
  expect(overlay.style.getPropertyValue('--sc-surface')).toBeTruthy();
  shell.close(null);
  await result;
  source.update('fluent', true);
  expect(overlay.style.getPropertyValue('--sc-surface')).toBe('');
});

afterEach(() => {
  document.body.replaceChildren();
  document.head.replaceChildren();
  document.documentElement.style.removeProperty('color-scheme');
  document.documentElement.removeAttribute('data-sc-visual-style');
});

test('Spectrum form styling preserves focused text, validation, literal labels and submission across style changes', async () => {
  publishVisualStyle(document, 'spectrum');
  const modal = new InputModal({ adapter });
  const result = modal.open({
    title: '<img src=x> Name',
    initialValue: 'bad',
    validate: (value) => (value === 'bad' ? '<b>Use another name</b>' : ''),
  });
  const input = modal.elements.input;
  const overlay = document.querySelector<HTMLElement>('.sugarcubes-input-overlay');
  const confirm = overlay?.querySelector<HTMLButtonElement>('.p-dialog-footer .p-button-primary');
  if (!input || !overlay || !confirm) throw new Error('Missing input modal controls');
  input.focus();
  input.setSelectionRange(1, 2);
  expect(overlay.dataset.scModalAppearance).toBe('spectrum');
  expect(getComputedStyle(overlay).getPropertyValue('--sc-field-focus-shadow')).toBe('none');
  confirm.click();
  expect(overlay.querySelector('.sugarcubes-modal__error')?.textContent).toBe(
    '<b>Use another name</b>',
  );
  expect(overlay.querySelector('b, img')).toBeNull();
  publishVisualStyle(document, 'fluent');
  expect(overlay.dataset.scModalAppearance).toBe('fluent');
  expect(overlay.style.getPropertyValue('--sc-field-focus-shadow')).toContain('inset 0 -1px');
  expect(document.activeElement).toBe(input);
  expect(input.selectionStart).toBe(1);
  expect(input.selectionEnd).toBe(2);
  publishVisualStyle(document, 'spectrum');
  expect(overlay.querySelector('input')).toBe(input);
  input.value = '<literal name>';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  confirm.click();
  await expect(result).resolves.toBe('<literal name>');
  expect(overlay.dataset.scModalAppearance).toBeUndefined();
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
  const dark = overlay?.style.getPropertyValue('--sc-surface');
  expect(dark).toBeTruthy();
  document.documentElement.style.colorScheme = 'light';
  await Promise.resolve();
  expect(overlay?.style.getPropertyValue('--sc-surface')).not.toBe(dark);
  shell.close(null);
  await expect(result).resolves.toBeNull();
  expect(overlay?.style.getPropertyValue('--sc-surface')).toBe('');
  document.documentElement.style.colorScheme = 'dark';
  await Promise.resolve();
  expect(overlay?.style.getPropertyValue('--sc-surface')).toBe('');
});
