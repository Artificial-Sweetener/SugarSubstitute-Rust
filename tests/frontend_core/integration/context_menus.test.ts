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
/** Prove universal menus own safe rows, nested focus, dismissal and exclusive document lifetime. */
import { test, expect, jest } from '@jest/globals';
import { ContextMenu } from '../../../frontend/comfyui/ui/menus/ContextMenu.js';

test('renders non-interactive section labels literally without introducing another submenu level', () => {
  const menu = new ContextMenu(document);
  menu.open({
    label: 'Presets',
    anchor: { kind: 'pointer', x: 12, y: 12 },
    entries: [{ label: '<b>Global</b>', section: true }, { label: '960 x 1344' }],
  });
  expect(document.querySelector('[data-sc-menu-section]')?.textContent).toBe('<b>Global</b>');
  expect(document.querySelector('[data-sc-menu-section] b')).toBeNull();
  expect(document.querySelectorAll('[role=menuitem]')).toHaveLength(1);
  expect(document.activeElement?.textContent).toBe('960 x 1344');
  menu.close();
});

test('supports literal labels, icons, checked rows and isolated submenu keyboard actions', () => {
  const trigger = document.createElement('button');
  document.body.append(trigger);
  trigger.focus();
  const action = jest.fn();
  const closed = jest.fn();
  const menu = new ContextMenu(document);
  menu.open({
    label: 'Fixture actions',
    trigger,
    anchor: { kind: 'pointer', x: 100, y: 50 },
    closed,
    entries: [
      { label: 'Disabled', disabled: true, select: action },
      {
        label: '<img src=x>',
        icon: (doc) => doc.createElement('span'),
        children: [
          { label: 'First', checked: true, select: action },
          { label: 'Second', checked: false, select: action },
        ],
      },
    ],
  });
  const parent = document.activeElement;
  expect(parent?.textContent).toBe('<img src=x>');
  expect(document.querySelector('img')).toBeNull();
  expect(parent?.querySelector('[slot=start]')?.getAttribute('aria-hidden')).toBe('true');
  parent?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  expect(document.querySelectorAll('[role=menu]')).toHaveLength(2);
  expect(document.activeElement?.textContent).toBe('First');
  document.activeElement?.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }),
  );
  expect(document.activeElement?.textContent).toBe('Second');
  document.activeElement?.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true }),
  );
  expect(document.querySelectorAll('[role=menu]')).toHaveLength(1);
  expect(document.activeElement).toBe(parent);
  parent?.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
  document.activeElement?.dispatchEvent(
    new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }),
  );
  expect(action).toHaveBeenCalledTimes(1);
  expect(closed).toHaveBeenCalledTimes(1);
  expect(document.querySelector('[role=menu]')).toBeNull();
  expect(document.activeElement).toBe(trigger);
  trigger.remove();
});

test('one document cannot retain competing menus and internal scrolling does not dismiss', () => {
  const first = new ContextMenu(document);
  const second = new ContextMenu(document);
  const closed = jest.fn();
  first.open({
    label: 'First',
    anchor: { kind: 'pointer', x: 0, y: 0 },
    entries: [{ label: 'A' }],
    closed,
  });
  second.open({
    label: 'Second',
    anchor: { kind: 'pointer', x: 20, y: 20 },
    entries: [{ label: 'B' }],
  });
  expect(first.isOpen()).toBe(false);
  expect(closed).toHaveBeenCalledTimes(1);
  expect(document.querySelectorAll('[role=menu]')).toHaveLength(1);
  document.querySelector('[role=menu]')?.dispatchEvent(new Event('scroll'));
  expect(second.isOpen()).toBe(true);
  document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }));
  expect(second.isOpen()).toBe(false);
});

test('reconciles checked choices without replacing the floating surface', () => {
  const menu = new ContextMenu(document);
  menu.open({
    label: 'Choices',
    anchor: { kind: 'pointer', x: 20, y: 20 },
    entries: [{ label: 'Visible', checked: false, selection: 'multiple', keepOpen: true }],
  });
  const surface = document.querySelector<HTMLElement>('[role=menu]');
  if (!surface) throw new Error('Missing menu');
  surface.scrollTop = 23;
  menu.updateEntries([{ label: 'Visible', checked: true, selection: 'multiple', keepOpen: true }]);
  expect(document.querySelector('[role=menu]')).toBe(surface);
  expect(surface.scrollTop).toBe(23);
  expect(document.activeElement?.getAttribute('aria-checked')).toBe('true');
  menu.close();
});
