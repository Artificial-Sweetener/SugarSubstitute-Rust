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
/** Style the browser image viewport independently of reusable overlay controls. */
/** Install static transform layers without allocating images or modifying host graph layout. */
export function ensureBrowserCanvasStyles(document) {
    if (document.getElementById('sugarcubes-output-browser-styles'))
        return;
    const style = document.createElement('style');
    style.id = 'sugarcubes-output-browser-styles';
    style.textContent = `
    .sc-output-viewport { position:absolute; inset:0; overflow:hidden; touch-action:none;
      background:var(--sugarcubes-canvas-background,#151515); cursor:grab; outline:none;
      overscroll-behavior:contain; user-select:none; }
    .sc-output-viewport:focus-visible { outline:2px solid var(--p-primary-color,var(--ring,#df258e)); outline-offset:-2px; }
    .sc-output-viewport.sc-output-dragging { cursor:grabbing; }
    .sc-output-image-layer { position:absolute; inset:0; overflow:hidden; pointer-events:none; }
    .sc-output-image { position:absolute; left:0; top:0; display:block;
      max-width:none; max-height:none; object-fit:fill; user-select:none; }
  `;
    document.head.append(style);
}
