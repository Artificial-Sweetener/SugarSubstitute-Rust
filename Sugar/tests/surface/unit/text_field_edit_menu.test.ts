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
import {
  FIELD_MENU_OWNER,
  TextFieldEditMenu,
} from '../../../frontend/comfyui/ui/menus/TextFieldEditMenu.js';
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

test('a control that owns its field menu keeps it; non-text inputs are never touched', () => {
  const menu = new TextFieldEditMenu(document, host(true), jest.fn());
  menu.setup();
  const picker = document.createElement('div');
  picker.setAttribute(FIELD_MENU_OWNER, '');
  const owned = document.createElement('input');
  const ownMenu = jest.fn();
  picker.addEventListener('contextmenu', ownMenu);
  picker.append(owned);
  const checkbox = document.createElement('input');
  checkbox.type = 'checkbox';
  document.body.append(picker, checkbox);
  rightClick(owned);
  expect(labels()).toEqual([]);
  expect(ownMenu).toHaveBeenCalledTimes(1);
  expect(rightClick(checkbox).defaultPrevented).toBe(false);
  menu.dispose();
});

test('a node that stops the right-click on its way back still leaves the field its edit menu', () => {
  const menu = new TextFieldEditMenu(document, host(true), jest.fn());
  menu.setup();
  // ComfyUI's Nodes 2.0 card cancels and stops every right-click inside it for its node menu.
  const node = document.createElement('div');
  const nodeMenu = jest.fn((event: Event) => {
    event.preventDefault();
    event.stopPropagation();
  });
  node.addEventListener('contextmenu', nodeMenu);
  const steps = document.createElement('input');
  node.append(steps);
  document.body.append(node);
  expect(rightClick(steps).defaultPrevented).toBe(true);
  expect(labels()).toEqual(['Undo', 'Redo', 'Cut', 'Copy', 'Paste', 'Select all']);
  expect(nodeMenu).not.toHaveBeenCalled();
  menu.dispose();
});

test('a document-level menu that answers first, such as Dimensions, keeps the right-click', () => {
  const menu = new TextFieldEditMenu(document, host(true), jest.fn());
  menu.setup();
  const dimensions = (event: Event): void => {
    event.preventDefault();
    event.stopImmediatePropagation();
  };
  document.addEventListener('contextmenu', dimensions, true);
  const width = document.createElement('input');
  document.body.append(width);
  rightClick(width);
  expect(labels()).toEqual([]);
  document.removeEventListener('contextmenu', dimensions, true);
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
