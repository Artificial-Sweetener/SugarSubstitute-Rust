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
/** Keep startup recovery distinct from settings navigation and subsequent authored workflow ownership. */
import { DesktopStartupIntent } from '../../../frontend/comfyui/ui/workflow/application/DesktopStartupIntent.js';

test('settings-only interruption survives ordinary initial startup and later authored work enables capture while superseding restore', () => {
  const intent = new DesktopStartupIntent();
  intent.settingsOpened();
  intent.workflow('initial', true, false);
  intent.committed('initial');
  intent.workflow('initial', false, false);
  expect(intent.captureAllowed()).toBe(false);
  expect(intent.restoreAllowed()).toBe(false);
  intent.authored();
  expect(intent.captureAllowed()).toBe(true);
  expect(intent.restoreAllowed()).toBe(false);
});

test('history or activation through the actual load boundary supersedes restore even when runtime context identity is unchanged', () => {
  const intent = new DesktopStartupIntent();
  intent.committed('workflow-A');
  intent.workflow('loading', true, false);
  intent.workflow('workflow-A', false, false);
  expect(intent.restoreAllowed()).toBe(false);
  expect(intent.captureAllowed()).toBe(true);
});

test('controlled fresh materialization bindings seed the next context without claiming user intent', () => {
  const intent = new DesktopStartupIntent();
  intent.committed('initial');
  intent.workflow('controlled-loading', true, true);
  intent.workflow('fresh-restored', false, true);
  intent.restored('fresh-restored');
  intent.workflow('fresh-restored', false, false);
  expect(intent.restoreAllowed()).toBe(true);
  expect(intent.captureAllowed()).toBe(true);
  intent.workflow('explicit-new-workflow', false, false);
  expect(intent.restoreAllowed()).toBe(false);
});

test('after host acknowledgement normal settings navigation does not suspend autosave', () => {
  const intent = new DesktopStartupIntent();
  intent.committed('restored');
  intent.acknowledged();
  intent.settingsOpened();
  expect(intent.captureAllowed()).toBe(true);
});
