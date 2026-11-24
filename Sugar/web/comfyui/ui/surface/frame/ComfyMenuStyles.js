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
 * In the SugarSubstitute frame, ComfyUI's own menus take the active style's menu look
 * (`MENU_LOOKS`), so they read as one with SugarCubes' menus: the classic graph menu (LiteGraph's
 * `.litecontextmenu`) and its newer menus (PrimeVue's context, tiered and plain menus, through
 * their design tokens). Their content, icons and behaviour stay ComfyUI's; outside the frame they
 * are left exactly as ComfyUI draws them.
 */
import { menuTokenCss } from '../../menus/MenuTokens.js';
const FRAME = ':root[data-sc-substitute-frame]';
const PRIME_MENUS = ['contextmenu', 'tieredmenu', 'menu'];
/** PrimeVue menu design tokens pointed at the menu look. */
function primeMenuTokens() {
    return PRIME_MENUS.map((menu) => `
    --p-${menu}-background:var(--sc-menu-surface);
    --p-${menu}-border-color:var(--sc-menu-stroke);
    --p-${menu}-border-radius:var(--sc-menu-radius);
    --p-${menu}-shadow:var(--sc-menu-shadow);
    --p-${menu}-color:var(--sc-menu-text);
    --p-${menu}-list-padding:var(--sc-menu-padding);
    --p-${menu}-item-padding:var(--sc-menu-row-padding);
    --p-${menu}-item-border-radius:var(--sc-menu-row-radius);
    --p-${menu}-item-color:var(--sc-menu-text);
    --p-${menu}-item-focus-color:var(--sc-menu-text);
    --p-${menu}-item-active-color:var(--sc-menu-text);
    --p-${menu}-item-focus-background:var(--sc-menu-row-hover);
    --p-${menu}-item-active-background:var(--sc-menu-row-pressed);
    --p-${menu}-item-icon-color:var(--sc-menu-text);
    --p-${menu}-item-icon-focus-color:var(--sc-menu-text);
    --p-${menu}-separator-border-color:var(--sc-menu-separator);`).join('');
}
/** ComfyUI's menus in the frame's menu look. */
export const comfyMenuCss = `
  ${menuTokenCss()}
  ${FRAME} {${primeMenuTokens()} }
  ${FRAME} :is(.p-contextmenu,.p-tieredmenu,.p-menu) { font:var(--sc-menu-font); }
  ${FRAME} :is(.p-contextmenu-item-content,.p-tieredmenu-item-content,.p-menu-item-content) {
    min-height:var(--sc-menu-row-height); }
  ${FRAME} :is(.p-contextmenu-item,.p-tieredmenu-item,.p-menu-item).p-disabled { opacity:1; }
  ${FRAME} :is(.p-contextmenu-item,.p-tieredmenu-item,.p-menu-item).p-disabled :is(
    .p-contextmenu-item-content,.p-tieredmenu-item-content,.p-menu-item-content) {
    color:var(--sc-menu-text-disabled); background:transparent; }
  ${FRAME} .litecontextmenu { box-sizing:border-box; padding:var(--sc-menu-padding) !important;
    border:1px solid var(--sc-menu-stroke) !important; border-radius:var(--sc-menu-radius) !important;
    background:var(--sc-menu-surface) !important; box-shadow:var(--sc-menu-shadow) !important;
    color:var(--sc-menu-text) !important; font:var(--sc-menu-font) !important; filter:none !important; }
  ${FRAME} .litecontextmenu .litemenu-title { margin:0 0 4px; padding:var(--sc-menu-row-padding);
    background:transparent !important; color:var(--sc-menu-text-disabled) !important; font:inherit; }
  ${FRAME} .litecontextmenu .litemenu-entry { display:flex; align-items:center; min-height:var(--sc-menu-row-height);
    box-sizing:border-box; margin:0; padding:var(--sc-menu-row-padding) !important; border:0 !important;
    border-radius:var(--sc-menu-row-radius); background:transparent !important; color:var(--sc-menu-text) !important; }
  ${FRAME} .litecontextmenu .litemenu-entry:hover:not(.disabled):not(.separator) {
    background:var(--sc-menu-row-hover) !important; }
  ${FRAME} .litecontextmenu .litemenu-entry:active:not(.disabled):not(.separator) {
    background:var(--sc-menu-row-pressed) !important; }
  ${FRAME} .litecontextmenu .litemenu-entry.disabled { color:var(--sc-menu-text-disabled) !important; opacity:1; }
  ${FRAME} .litecontextmenu .litemenu-entry.separator { display:block; min-height:0; height:1px; margin:4px 0;
    padding:0 !important; background:var(--sc-menu-separator) !important; }`;
