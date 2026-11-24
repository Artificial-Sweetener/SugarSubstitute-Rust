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
/**
 * Bring SugarCubes' menus in the Fluent style in line with WinUI's: Fluent's web menu item paints
 * a disabled row with its disabled background, a near-black box in the dark theme, where WinUI
 * leaves a disabled row on the menu surface and only dims its text and icon.
 */
const STYLE_ID = 'sugarcubes-context-menu-styles';
/** The menu rules SugarCubes adds over Fluent's menu components. */
export const contextMenuCss = `
  html[data-sc-visual-style="fluent"] .sc-context-menu fluent-menu-item:is([disabled],[aria-disabled="true"]) {
    background-color:transparent; }`;
/** Install the rules once per document. */
export function ensureContextMenuStyles(document) {
    if (document.getElementById(STYLE_ID))
        return;
    const style = document.createElement('style');
    style.id = STYLE_ID;
    style.textContent = contextMenuCss;
    document.head.append(style);
}
