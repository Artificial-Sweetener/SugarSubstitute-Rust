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
/** Paint identified action and choice menus from one PhotoPro surface contract. */
/** Give every ordinary menu Photoshop's light, square presentation. */
export function photoProMenuCss() {
    const surface = 'html[data-sc-visual-style="photopro"] :is([data-sc-menu-surface="actions"],[data-sc-menu-surface="choices"][data-sc-style-popup])';
    const row = `${surface} :is([role^="menuitem"],[role="option"])`;
    return `
    ${surface} {
      box-sizing:border-box; padding:2px 0; border:1px solid #8a8a8a;
      border-radius:0; background:#f0f0f0; color:#111; color-scheme:light;
      box-shadow:1px 2px 5px #0007; font:12px/18px Arial,sans-serif;
      --sc-font-family:Arial,sans-serif; --sc-border-subtle:#bdbdbd;
    }
    ${surface} [data-testid="widget-select-default-viewport"] { gap:0; padding:0; }
    ${row} {
      position:relative; display:flex; align-items:center; box-sizing:border-box; min-height:18px;
      gap:7px; padding:0 20px; border:0; border-radius:0;
      color:inherit; background:transparent; font:inherit; outline-offset:-1px;
    }
    ${row}:is([aria-checked="true"],[aria-selected="true"],[data-state="checked"]) { background:transparent; }
    ${row}:is(:hover,:focus-visible,[data-highlighted]):not([aria-disabled="true"]) { background:#cce8ff; }
    ${row}:is([aria-disabled="true"],:disabled) { color:#888; }
    ${row}:is([aria-checked="true"],[aria-selected="true"],[data-state="checked"])::before {
      content:'✓'; position:absolute; left:5px; color:#111;
    }
    ${surface} [role="option"] > [aria-hidden="true"] { display:none; }
    ${surface} [role="option"] > :not([aria-hidden="true"]) { color:#111 !important; opacity:1; }
    ${surface} [role="option"][aria-disabled="true"] > :not([aria-hidden="true"]) { color:#888 !important; }
    ${surface} [role="separator"] {
      height:1px !important; margin:3px 0 !important; background:#bdbdbd !important;
    }
  `;
}
