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
/** Register Microsoft's official menu components at the browser dependency boundary. */
import { MenuList } from '@fluentui/web-components/menu-list/class.js';
import { definition as MenuListDefinition } from '@fluentui/web-components/menu-list/definition.js';
import { MenuItem } from '@fluentui/web-components/menu-item/class.js';
import { definition as MenuItemDefinition } from '@fluentui/web-components/menu-item/definition.js';

if (!customElements.get('fluent-menu-list')) await MenuList.define(MenuListDefinition);
if (!customElements.get('fluent-menu-item')) await MenuItem.define(MenuItemDefinition);

/** Create an official Fluent list while the shared owner controls its floating lifecycle. */
export function createFluentMenuList(document: Document): HTMLElement {
  return document.createElement('fluent-menu-list');
}

/** Create an official Fluent row with slots for icons, content and accessory text. */
export function createFluentMenuItem(document: Document): HTMLElement {
  return document.createElement('fluent-menu-item');
}
