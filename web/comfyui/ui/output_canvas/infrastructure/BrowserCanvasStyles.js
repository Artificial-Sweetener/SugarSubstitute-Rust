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
    .sc-output-overview { position:relative; min-height:0; overflow:auto; overscroll-behavior:contain;
      scrollbar-gutter:stable; color:var(--fg-color,#ddd); }
    .sc-output-overview[hidden] { display:none; }
    .sc-output-overview-track { position:relative; min-height:100%; }
    .sc-output-overview-window { position:absolute; left:0; right:0; display:grid; }
    .sc-output-overview-cell { margin:6px; min-width:0; min-height:0; display:flex; flex-direction:column;
      gap:6px; padding:8px; border-radius:8px; border:1px solid var(--interface-stroke,#454545);
      color:inherit; background:var(--sugarcubes-card-surface,var(--comfy-menu-bg,#202020)); cursor:pointer; }
    .sc-output-overview-cell:focus-visible { outline:2px solid var(--p-primary-color,var(--ring,#df258e)); outline-offset:-2px; }
    .sc-output-overview-cell > span { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; width:100%; text-align:left; }
    .sc-output-overview-cell-status { font-size:12px; opacity:.8; }
    .sc-output-overview-image { flex:1; min-height:0; width:100%; display:flex; align-items:center; justify-content:center; }
    .sc-output-overview-thumbnail { max-width:100%; max-height:100%; object-fit:contain; }
    .sc-output-overview-mode { display:grid; grid-template-rows:auto minmax(0,1fr) auto; }
    .sc-output-overview-mode .sc-output-overlay-root,.sc-output-overview-mode .sc-output-overlay { display:contents; }
    .sc-output-overview-mode .sc-output-navigation { position:relative; inset:auto; margin:12px; grid-row:1; }
    .sc-output-overview-mode .sc-output-overview { grid-row:2; z-index:1; }
    .sc-output-overview-mode .sc-output-tabs { position:relative; inset:auto; transform:none; justify-self:center; margin:12px; grid-row:3; }
    .sc-output-overview-mode .sc-output-status { display:none; }
  `;
    document.head.append(style);
}
