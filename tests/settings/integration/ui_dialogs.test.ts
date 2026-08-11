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
import { describe, expect, test, beforeEach } from '@jest/globals';
import { ConfirmDialog } from '../../../frontend/comfyui/ui/dialogs/ConfirmDialog.js';
import { FormModal } from '../../../frontend/comfyui/ui/dialogs/FormModal.js';
import { InputModal } from '../../../frontend/comfyui/ui/dialogs/InputModal.js';
import { ModalService } from '../../../frontend/comfyui/ui/dialogs/ModalService.js';
import { SelectionModal } from '../../../frontend/comfyui/ui/dialogs/SelectionModal.js';
import { VersionDialog } from '../../../frontend/comfyui/ui/dialogs/VersionDialog.js';
import { StorageService } from '../../../frontend/comfyui/ui/core/StorageService.js';

const adapter = {
  getDocument: () => document,
  getWindow: () => window,
};

type TestElement = HTMLElement & { value: string; disabled: boolean; checked: boolean };

function requiredElement(selector: string): TestElement {
  const element = document.querySelector<TestElement>(selector);
  if (!element) throw new Error(`Missing test element: ${selector}`);
  return element;
}

beforeEach(() => {
  document.body.innerHTML = '';
  localStorage.clear();
});

describe('dialogs', () => {
  test('confirm dialog resolves true on confirm', async () => {
    const dialog = new ConfirmDialog({ adapter });
    const promise = dialog.open({
      title: 'Confirm delete',
      message: 'Delete cube?',
      confirmLabel: 'Delete',
    });

    const confirmButton = requiredElement('.sugarcubes-confirm__confirm');
    expect(confirmButton).not.toBeNull();
    confirmButton.click();

    await expect(promise).resolves.toBe(true);
  });

  test('confirm dialog resolves false on escape', async () => {
    const dialog = new ConfirmDialog({ adapter });
    const promise = dialog.open({
      title: 'Confirm',
      message: 'Press escape',
    });

    const event = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true });
    window.dispatchEvent(event);

    await expect(promise).resolves.toBe(false);
  });

  test('input modal resolves trimmed values and blocks invalid submit', async () => {
    const dialog = new InputModal({ adapter });
    const promise = dialog.open({
      title: 'Import',
      label: 'Cube id',
      initialValue: '  local/test/demo.cube  ',
      normalizeValue: (value) => value.trim(),
      validate: (value) => (value.includes('.cube') ? '' : 'Cube id is invalid.'),
      confirmLabel: 'Import',
    });

    const input = requiredElement('.sugarcubes-input-dialog input');
    const confirmButton = requiredElement('.sugarcubes-input-dialog button:last-child');
    expect(input).not.toBeNull();
    input.value = 'invalid';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    confirmButton.click();
    expect(document.querySelector('.sugarcubes-modal__error')?.textContent).toContain(
      'Cube id is invalid.',
    );

    input.value = ' local/test/demo.cube ';
    input.dispatchEvent(new Event('input', { bubbles: true }));
    confirmButton.click();

    await expect(promise).resolves.toBe('local/test/demo.cube');
  });

  test('form modal validates required fields and returns values', async () => {
    const dialog = new FormModal({ adapter });
    const promise = dialog.open({
      title: 'Add Cube Pack',
      confirmLabel: 'Add Pack',
      fields: [
        {
          key: 'repoRef',
          label: 'Source repository',
          required: true,
          normalizeValue: (value) => value.trim(),
        },
        {
          key: 'branch',
          label: 'Branch',
          required: true,
          initialValue: 'main',
          normalizeValue: (value) => value.trim(),
        },
        {
          key: 'defaultBaseRepo',
          label: 'Set as default base repo',
          type: 'checkbox',
          initialValue: true,
        },
      ],
    });

    const inputs = document.querySelectorAll<HTMLInputElement>('.sugarcubes-form-dialog input');
    const confirmButton = requiredElement('.sugarcubes-form-dialog button:last-child');
    inputs[0].value = '';
    inputs[0].dispatchEvent(new Event('input', { bubbles: true }));
    confirmButton.click();
    expect(document.querySelector('.sugarcubes-modal__field-error')?.textContent).toContain(
      'Source repository is required.',
    );

    inputs[0].value = 'Artificial-Sweetener/Base-Cubes';
    inputs[0].dispatchEvent(new Event('input', { bubbles: true }));
    confirmButton.click();

    await expect(promise).resolves.toEqual({
      repoRef: 'Artificial-Sweetener/Base-Cubes',
      branch: 'main',
      defaultBaseRepo: true,
    });
  });

  test('selection modal requires a selection and resolves the selected item', async () => {
    const dialog = new SelectionModal({ adapter });
    const promise = dialog.open({
      title: 'Delete Local Flavor',
      confirmLabel: 'Delete Flavor',
      items: [
        { value: 'portrait', label: 'Portrait', description: 'portrait' },
        { value: 'cinematic', label: 'Cinematic', description: 'cinematic' },
      ],
    });

    const confirmButton = requiredElement('.sugarcubes-selection-dialog button:last-child');
    expect(confirmButton.disabled).toBe(true);

    const radio = requiredElement('.sugarcubes-selection-dialog input[value="cinematic"]');
    radio.checked = true;
    radio.dispatchEvent(new Event('change', { bubbles: true }));
    confirmButton.click();

    await expect(promise).resolves.toBe('cinematic');
  });

  test('confirm dialog renders markup-like text literally', async () => {
    const dialog = new ConfirmDialog({ adapter });
    const promise = dialog.open({
      title: 'Confirm',
      message: ['Delete "<img src=x onerror=1>"?', 'This cannot be undone.'],
    });

    const paragraphs = Array.from(document.querySelectorAll('.sugarcubes-confirm__message p')).map(
      (node) => node.textContent,
    );
    expect(paragraphs).toEqual(['Delete "<img src=x onerror=1>"?', 'This cannot be undone.']);
    expect(document.querySelector('.sugarcubes-confirm__message img')).toBeNull();

    dialog.close(false);
    await expect(promise).resolves.toBe(false);
  });

  test('modal service alert uses SugarCubes modal without a cancel button', async () => {
    const dialogs = new ModalService({ adapter });
    const promise = dialogs.alert({
      title: 'Create failed',
      message: ['Unable to serialize the current graph.'],
    });

    const dialog = requiredElement('.sugarcubes-confirm-dialog');
    const cancelButton = dialog.querySelector<HTMLButtonElement>('button:first-child');
    const confirmButton = dialog.querySelector<HTMLButtonElement>('button:last-child');
    if (!cancelButton || !confirmButton) throw new Error('Missing alert actions');
    expect(dialog.textContent).toContain('Create failed');
    expect(dialog.textContent).toContain('Unable to serialize the current graph.');
    expect(cancelButton.style.display).toBe('none');
    expect(confirmButton.textContent).toBe('OK');

    confirmButton.click();
    await expect(promise).resolves.toBe(true);
  });

  test('modal service historical save choice resolves fork and renders text literally', async () => {
    const dialogs = new ModalService({ adapter });
    const promise = dialogs.chooseHistoricalVersionSaveAction({
      entries: [{ defaultAlias: '<img src=x onerror=1>', sourceVersion: '1.0.1' }],
    });

    const dialog = requiredElement('.sugarcubes-historical-save-dialog');
    expect(dialog).not.toBeNull();
    expect(dialog.textContent).toContain('Save older version?');
    expect(dialog.textContent).toContain('This SugarCube was loaded from an older version.');
    expect(dialog.textContent).toContain('Fork instead');
    expect(dialog.querySelector('img')).toBeNull();

    const forkButton = Array.from(dialog.querySelectorAll('button')).find(
      (button) => button.textContent === 'Fork instead',
    );
    if (!forkButton) throw new Error('Missing fork action');
    forkButton.click();

    await expect(promise).resolves.toBe('fork');
  });

  test('default review presents only aggregate choices and hides repository identity', async () => {
    const dialogs = new ModalService({ adapter });
    const promise = dialogs.reviewImplementationDefaults([
      {
        cubeId: 'local/me/prompt.cube',
        displayName: 'Prompt <img src=x onerror=1>',
        fingerprint: 'fingerprint',
        requiresDefaultDecision: true,
        overwriteDefaultCount: 1,
        promptDefaultCount: 1,
        changes: [
          {
            section: 'Defaults',
            label: 'Prompt',
            path: 'flavors.authored.default.values.prompt.text',
            previousValue: '',
            proposedValue: 'authored prompt',
            previousExists: true,
            proposedExists: true,
            decision: 'save_prompt_fields',
          },
          {
            section: 'Defaults',
            label: 'Width',
            path: 'flavors.authored.default.values.size.width',
            previousValue: 960,
            proposedValue: 1080,
            previousExists: true,
            proposedExists: true,
            decision: 'overwrite_defaults',
          },
        ],
      },
    ]);

    const dialog = requiredElement('.sugarcubes-default-review-dialog');
    const checkboxes = dialog.querySelectorAll<HTMLInputElement>('input[type="checkbox"]');
    if (checkboxes.length !== 2) throw new Error('Missing aggregate default choices');
    const [overwriteDefaults, savePromptFields] = checkboxes;
    if (!overwriteDefaults || !savePromptFields) throw new Error('Missing aggregate choices');
    expect(overwriteDefaults.checked).toBe(false);
    expect(savePromptFields.checked).toBe(false);
    expect(dialog.textContent).toContain('Overwrite current defaults');
    expect(dialog.textContent).toContain('Save prompt fields as cube defaults');
    expect(dialog.textContent).not.toContain('local/me/prompt.cube');
    const details = requiredElement('.sugarcubes-default-review__details') as HTMLElement;
    expect(details.hidden).toBe(true);
    expect(details.textContent).toContain('Width');
    expect(details.textContent).toContain('960');
    expect(dialog.querySelector('img')).toBeNull();

    overwriteDefaults.click();
    savePromptFields.click();
    requiredElement('.sugarcubes-default-review-dialog button:last-child').click();

    await expect(promise).resolves.toEqual({
      'local/me/prompt.cube': {
        overwriteDefaults: true,
        savePromptFields: true,
      },
    });
  });

  test('More info reveals the complete read-only implementation diff', async () => {
    const dialogs = new ModalService({ adapter });
    const promise = dialogs.reviewImplementationDefaults([
      {
        cubeId: 'local/me/prompt.cube',
        displayName: 'Prompt by Region',
        fingerprint: 'fingerprint',
        requiresDefaultDecision: true,
        overwriteDefaultCount: 1,
        promptDefaultCount: 1,
        changes: [
          {
            section: 'Defaults',
            label: 'Prompt',
            path: 'flavors.authored.default.values.prompt.text',
            previousValue: '',
            proposedValue: '<img src=x onerror=1>',
            previousExists: true,
            proposedExists: true,
            decision: 'save_prompt_fields',
          },
          {
            section: 'Defaults',
            label: 'Width',
            path: 'flavors.authored.default.values.size.width',
            previousValue: 960,
            proposedValue: 1080,
            previousExists: true,
            proposedExists: true,
            decision: 'overwrite_defaults',
          },
          {
            section: 'Nodes',
            label: 'Regional latent',
            path: 'implementation.nodes.regional_latent',
            previousValue: null,
            proposedValue: { class_type: 'EmptyLatentImage' },
            previousExists: false,
            proposedExists: true,
            decision: 'always',
          },
        ],
      },
    ]);

    const dialog = requiredElement('.sugarcubes-default-review-dialog');
    const details = requiredElement('.sugarcubes-default-review__details') as HTMLElement;
    expect(details.hidden).toBe(true);
    const moreInfo = Array.from(dialog.querySelectorAll('button')).find(
      (button) => button.textContent === 'More info',
    );
    if (!moreInfo) throw new Error('Missing More info action');
    moreInfo.click();

    expect(details.hidden).toBe(false);
    expect(details.textContent).toContain('Width');
    expect(details.textContent).toContain('960 → 1080');
    expect(details.textContent).toContain('Regional latent');
    expect(details.textContent).toContain('<img src=x onerror=1>');
    expect(dialog.querySelector('img')).toBeNull();
    expect(dialog.querySelectorAll('input[type="checkbox"]')).toHaveLength(2);

    requiredElement('.sugarcubes-default-review-dialog button:last-child').click();
    await expect(promise).resolves.toEqual({
      'local/me/prompt.cube': {
        overwriteDefaults: false,
        savePromptFields: false,
      },
    });
  });

  test('version dialog persists dismissal', async () => {
    const storage = new StorageService({
      getStorage: () => localStorage,
    });
    const dialog = new VersionDialog({ adapter, storage });
    const suggestions = [
      {
        default_alias: 'Cube A',
        current_version: '1.0.0',
        suggested_version: '1.0.1',
        reason: 'Test',
      },
    ];

    const promise = dialog.open(suggestions);
    const checkbox = requiredElement('.sugarcubes-version-dialog__dismiss input');
    const confirmButton = requiredElement('.sugarcubes-version-dialog button:last-child');
    expect(checkbox).not.toBeNull();
    expect(confirmButton).not.toBeNull();
    checkbox.checked = true;
    confirmButton.click();

    await expect(promise).resolves.toBe(true);
    expect(localStorage.getItem('sugarcubes.version_prompt.dismissed')).toBe('true');

    await expect(dialog.open(suggestions)).resolves.toBe(false);
  });

  test('version dialog suggestion text stays literal', () => {
    const storage = new StorageService({
      getStorage: () => localStorage,
    });
    const dialog = new VersionDialog({ adapter, storage });
    dialog.open([
      {
        default_alias: '<b>Cube A</b>',
        current_version: '1.0.0',
        suggested_version: '1.0.1',
        reason: '<img src=x onerror=1>',
      },
    ]);

    const item = document.querySelector('.sugarcubes-version-dialog li');
    expect(item?.textContent).toContain('<b>Cube A</b>');
    expect(item?.textContent).toContain('<img src=x onerror=1>');
    expect(document.querySelector('.sugarcubes-version-dialog img')).toBeNull();
  });
});
