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
/** Prove source controls retain literal text, unsupported status and recoverable workflow-fence errors. */
import { jest } from '@jest/globals';
import { SourceSavingDialog } from '../../../frontend/comfyui/ui/output_saving/SourceSavingDialog.js';
import type { SourceSavingEditSession } from '../../../frontend/comfyui/ui/output_saving/SourceSavingEditorPort.js';
import { savingPreferences } from '../support/SavingSettingsFixture.js';

test('keeps unsupported saving disabled and retains edits when a workflow changes before confirmation', async () => {
  const commit = jest.fn<SourceSavingEditSession['commit']>().mockImplementation(() => {
    throw new Error('The workflow changed.');
  });
  const session: SourceSavingEditSession = {
    targets: [
      {
        path: 'a',
        title: '<image source>',
        classType: 'SaveImage',
        kind: 'save',
        supported: true,
        preference: { enabled: true, label: null },
      },
      {
        path: 'b',
        title: 'Custom saver',
        classType: 'CustomSaver',
        kind: null,
        supported: false,
        preference: { enabled: true, label: null },
      },
    ],
    current: () => true,
    commit,
  };
  const dialog = new SourceSavingDialog(document, {
    getDocument: () => document,
    getWindow: () => window,
  });
  const result = dialog.open(session, savingPreferences);
  const unsupported = document.querySelector('[aria-label="Custom saver (b) · Save output"]');
  expect(unsupported).toBeInstanceOf(HTMLInputElement);
  expect(unsupported?.hasAttribute('disabled')).toBe(true);
  const label = [...document.querySelectorAll('input')].find(
    (input) => input.getAttribute('aria-label') === '<image source> (a) · Output label',
  );
  expect(
    [...document.querySelectorAll('input')].map((input) => input.getAttribute('aria-label')),
  ).toContain('<image source> (a) · Output label');
  if (!(label instanceof HTMLInputElement)) throw new Error('Missing label control.');
  label.value = '<new label>';
  expect(document.querySelector('image')).toBeNull();
  expect(document.body.textContent).toContain('no verified SugarCubes integration');
  const confirm = document.querySelector('.sugarcubes-modal__footer-actions button:last-child');
  if (!(confirm instanceof HTMLButtonElement)) throw new Error('Missing confirmation.');
  confirm.click();
  expect(document.body.textContent).toContain('The workflow changed.');
  expect(label.value).toBe('<new label>');
  const cancel = document.querySelector('.sugarcubes-modal__footer-actions button:first-child');
  if (!(cancel instanceof HTMLButtonElement)) throw new Error('Missing cancellation.');
  cancel.click();
  await result;
});
