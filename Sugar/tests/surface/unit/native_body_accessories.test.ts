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
/** Prove native fields retain their ownership while accessories participate in the same card geometry. */
import { jest } from '@jest/globals';
import { reconcileNativeNodeBodyAccessories } from '../../../frontend/comfyui/ui/surface/NativeNodeBodyAccessories.js';
import { CommandButtonMount } from '../../../frontend/comfyui/ui/surface/appearance/CommandButtonMount.js';
import { publishVisualStyle } from '../../../frontend/comfyui/ui/surface/appearance/VisualStyleBoundary.js';

afterEach(() => {
  document.body.replaceChildren();
  publishVisualStyle(document, 'comfy');
  jest.restoreAllMocks();
});

test('places previews before the native body and commands after it without moving or replacing fields', () => {
  const root = document.createElement('article'),
    native = document.createElement('div'),
    header = document.createElement('header'),
    body = document.createElement('div'),
    footer = document.createElement('footer'),
    field = document.createElement('input');
  native.className = 'lg-node';
  body.dataset.testid = 'node-body-1';
  body.append(field);
  native.append(header, body, footer);
  root.append(native);
  document.body.append(root);
  const previews = [document.createElement('section'), document.createElement('section')],
    commands = [document.createElement('fieldset'), document.createElement('fieldset')];
  reconcileNativeNodeBodyAccessories(root, previews, commands);
  expect([...native.children]).toEqual([header, ...previews, body, ...commands, footer]);
  field.focus();
  const move = jest.spyOn(native, 'insertBefore');
  reconcileNativeNodeBodyAccessories(root, previews, commands);
  expect(move).not.toHaveBeenCalled();
  expect(document.activeElement).toBe(field);
  expect(body.firstElementChild).toBe(field);
  const replacement = document.createElement('div');
  replacement.className = 'lg-node';
  const replacementBody = document.createElement('div');
  replacementBody.dataset.testid = 'node-body-1';
  replacement.append(replacementBody);
  native.replaceWith(replacement);
  reconcileNativeNodeBodyAccessories(root, previews, commands);
  expect([...replacement.children]).toEqual([...previews, replacementBody, ...commands]);
});

test('uses each provider actual button asset on live style changes and preserves command state', () => {
  publishVisualStyle(document, 'photopro');
  const action = jest.fn(),
    command = new CommandButtonMount(document, 'Browse…', action);
  document.body.append(command.element);
  expect(command.element.tagName).toBe('BUTTON');
  command.element.click();
  expect(action).toHaveBeenCalledTimes(1);
  command.setDisabled(true);
  for (const [style, tag] of [
    ['fluent', 'FLUENT-BUTTON'],
    ['spectrum', 'SP-ACTION-BUTTON'],
    ['comfy', 'FLUENT-BUTTON'],
    ['photopro', 'BUTTON'],
  ] as const) {
    publishVisualStyle(document, style);
    expect(command.element.tagName).toBe(tag);
    expect(command.element.textContent).toBe('Browse…');
    expect(command.element.hasAttribute('disabled')).toBe(true);
    command.element.click();
  }
  expect(action).toHaveBeenCalledTimes(1);
  command.setDisabled(false);
  command.element.click();
  expect(action).toHaveBeenCalledTimes(2);
  command.dispose();
});
