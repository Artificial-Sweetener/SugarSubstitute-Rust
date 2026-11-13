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
/** Adapt official menu components while the shared owner retains interaction policy. */
import { MenuList } from '@fluentui/web-components/menu-list/class.js';
import { definition as MenuListDefinition } from '@fluentui/web-components/menu-list/definition.js';
import { MenuItem } from '@fluentui/web-components/menu-item/class.js';
import { definition as MenuItemDefinition } from '@fluentui/web-components/menu-item/definition.js';
import '../surface/appearance/SpectrumThemeRuntime.js';
import { readOwnedVisualStyle } from '../surface/appearance/VisualStyleBoundary.js';

if (!customElements.get('fluent-menu-list')) await MenuList.define(MenuListDefinition);
if (!customElements.get('fluent-menu-item')) await MenuItem.define(MenuItemDefinition);

/** Suppress Adobe's independent selection and roving-focus owner inside shared menus. */
export function createMenuList(document: Document): HTMLElement {
  const style = readOwnedVisualStyle(document);
  const menu = document.createElement(
    style === 'spectrum'
      ? 'sp-menu'
      : style === 'photopro' || style === 'gimp'
        ? 'div'
        : 'fluent-menu-list',
  );
  if (menu.localName === 'sp-menu') menu.setAttribute('ignore', '');
  return menu;
}

/** Create an official row with slots for icons, content and accessory text. */
export function createMenuItem(document: Document): HTMLElement {
  const style = readOwnedVisualStyle(document);
  return document.createElement(
    style === 'spectrum'
      ? 'sc-spectrum-menu-item'
      : style === 'photopro' || style === 'gimp'
        ? 'div'
        : 'fluent-menu-item',
  );
}

/** Reflect shared selection semantics into the selected library's checkmark presentation. */
export function setMenuItemChecked(row: HTMLElement, checked: boolean): void {
  row.setAttribute('aria-checked', String(checked));
  if (checked)
    row.setAttribute(
      readOwnedVisualStyle(row.ownerDocument) === 'spectrum' ? 'selected' : 'checked',
      '',
    );
}

/** Route shared icons and accessories into each library's documented slots. */
export function menuItemSlot(document: Document, position: 'start' | 'end'): string {
  return ['photopro', 'gimp'].includes(readOwnedVisualStyle(document))
    ? ''
    : readOwnedVisualStyle(document) === 'spectrum'
      ? position === 'start'
        ? 'icon'
        : 'value'
      : position;
}
