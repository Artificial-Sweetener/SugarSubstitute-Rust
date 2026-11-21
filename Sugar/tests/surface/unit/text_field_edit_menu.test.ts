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
/** Prove plain text fields take SugarCubes' edit menu exactly where the host lets them. */
import { afterEach, expect, jest, test } from '@jest/globals';
import { TextFieldEditMenu } from '../../../frontend/comfyui/ui/menus/TextFieldEditMenu.js';
import type { HostDescriptionSource } from '../../../frontend/comfyui/ui/core/embedded/HostDescriptionState.js';

afterEach(() => document.body.replaceChildren());

/** A host that does or does not declare `editing.text-menus`. */
function host(textMenus: boolean): HostDescriptionSource {
  return {
    read: () => ({ declares: (name: string) => textMenus && name === 'editing.text-menus' }),
    subscribe: () => () => undefined,
  } as unknown as HostDescriptionSource;
}

function rightClick(target: Element): MouseEvent {
  const event = new MouseEvent('contextmenu', {
    bubbles: true,
    cancelable: true,
    clientX: 5,
    clientY: 5,
  });
  target.dispatchEvent(event);
  return event;
}

function labels(): string[] {
  return [...document.querySelectorAll('[data-sc-floating-surface] [role^="menuitem"]')].map(
    (row) => row.textContent?.trim() ?? '',
  );
}

test('with the host capability every text field takes the edit menu, ComfyUI fields included', () => {
  const menu = new TextFieldEditMenu(document, host(true), jest.fn());
  menu.setup();
  const comfy = document.createElement('textarea');
  comfy.value = 'a prompt';
  document.body.append(comfy);
  expect(rightClick(comfy).defaultPrevented).toBe(true);
  expect(labels()).toEqual(['Undo', 'Redo', 'Cut', 'Copy', 'Paste', 'Select all']);
  menu.dispose();
});

test('without it only fields on SugarCubes surfaces take the menu; the browser keeps the rest', () => {
  const menu = new TextFieldEditMenu(document, host(false), jest.fn());
  menu.setup();
  const comfy = document.createElement('input');
  document.body.append(comfy);
  expect(rightClick(comfy).defaultPrevented).toBe(false);
  const surface = document.createElement('div');
  surface.setAttribute('data-sc-style-theme-root', '');
  const own = document.createElement('input');
  surface.append(own);
  document.body.append(surface);
  expect(rightClick(own).defaultPrevented).toBe(true);
  menu.dispose();
});

test('a field whose own menu already answered keeps it; non-text inputs are never touched', () => {
  const menu = new TextFieldEditMenu(document, host(true), jest.fn());
  menu.setup();
  const owned = document.createElement('input');
  owned.addEventListener('contextmenu', (event) => event.preventDefault());
  const checkbox = document.createElement('input');
  checkbox.type = 'checkbox';
  document.body.append(owned, checkbox);
  rightClick(owned);
  expect(labels()).toEqual([]);
  expect(rightClick(checkbox).defaultPrevented).toBe(false);
  menu.dispose();
});

test('rows follow the field: read-only takes no edits, nothing selected copies nothing', () => {
  const menu = new TextFieldEditMenu(document, host(true), jest.fn());
  menu.setup();
  const field = document.createElement('input');
  field.value = 'width';
  field.readOnly = true;
  document.body.append(field);
  field.setSelectionRange(0, 0);
  rightClick(field);
  const disabled = [...document.querySelectorAll('[data-sc-floating-surface] [role^="menuitem"]')]
    .filter((row) => row.getAttribute('aria-disabled') === 'true')
    .map((row) => row.textContent?.trim());
  expect(disabled).toEqual(['Undo', 'Redo', 'Cut', 'Copy', 'Paste']);
  menu.dispose();
});
