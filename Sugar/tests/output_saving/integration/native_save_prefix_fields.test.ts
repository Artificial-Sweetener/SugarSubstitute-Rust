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
/** Prove managed prefix presentation preserves values and filters unrelated popup activity. */
import { jest } from '@jest/globals';
import { NativeSavePrefixFields } from '../../../frontend/comfyui/ui/output_saving/NativeSavePrefixFields.js';
import type { UnknownRecord } from '../../../frontend/comfyui/ui/types/common.js';

test('leases native prefix editing without changing values or requesting graph discovery on popups', async () => {
  document.body.replaceChildren();
  const root = document.createElement('div');
  root.dataset.nodeId = '2';
  const input = document.createElement('input');
  input.setAttribute('aria-label', 'filename_prefix');
  input.value = 'Original prefix';
  root.append(input);
  document.body.append(root);
  const widget: UnknownRecord = { name: 'filename_prefix', value: 'Original prefix' };
  const source = {
    path: '2',
    node: { id: 2, type: 'SaveImage', widgets: [widget] },
    label: 'Save Image',
  };
  const fields = new NativeSavePrefixFields(document);
  fields.reconcile([source], true);
  expect(input.readOnly).toBe(true);
  expect(widget.disabled).toBe(true);
  expect(input.value).toBe('Original prefix');
  const query = jest.spyOn(document, 'querySelectorAll');
  query.mockClear();
  const popup = document.createElement('div');
  popup.setAttribute('role', 'dialog');
  popup.append(document.createElement('input'));
  document.body.append(popup);
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  expect(query).not.toHaveBeenCalled();
  fields.reconcile([source], false);
  expect(input.readOnly).toBe(false);
  expect(widget.disabled).toBeUndefined();
  expect(widget.value).toBe('Original prefix');
  fields.dispose();
  query.mockRestore();
  document.body.replaceChildren();
});

test('preserves an existing host read-only state and restores a removed workflow widget', () => {
  const root = document.createElement('div');
  root.dataset.nodeId = '9';
  const input = document.createElement('textarea');
  input.setAttribute('aria-label', 'filename_prefix');
  input.readOnly = true;
  root.append(input);
  document.body.append(root);
  const widget: UnknownRecord = { name: 'filename_prefix', value: 'Native', disabled: true };
  const fields = new NativeSavePrefixFields(document);
  fields.reconcile(
    [{ path: '9', node: { id: 9, type: 'SaveImage', widgets: [widget] }, label: 'Save' }],
    true,
  );
  fields.reconcile([], false);
  expect(input.readOnly).toBe(true);
  expect(widget.disabled).toBe(true);
  fields.dispose();
  root.remove();
});
