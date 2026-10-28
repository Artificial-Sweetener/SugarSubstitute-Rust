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
import { ensureCanvasViewportStyles } from './CanvasViewportStyles.js';
/** Install static transform layers without allocating images or modifying host graph layout. */
export function ensureBrowserCanvasStyles(document) {
    ensureCanvasViewportStyles(document);
    if (document.getElementById('sugarcubes-output-browser-styles'))
        return;
    const style = document.createElement('style');
    style.id = 'sugarcubes-output-browser-styles';
    style.textContent = `
    .sc-output-viewport { cursor:grab; }
    .sc-output-viewport.sc-output-dragging { cursor:grabbing; }
    .sc-output-image-layer { position:absolute; inset:0; overflow:hidden; pointer-events:none; }
    .sc-output-image { position:absolute; left:0; top:0; display:block; pointer-events:auto;
      max-width:none; max-height:none; object-fit:fill; user-select:none; }
    .sc-output-overview { position:absolute; inset:0; min-height:0; overflow:auto; overscroll-behavior:contain;
      color:var(--fg-color,#ddd); }
    .sc-output-overview[hidden] { display:none; }
    .sc-output-overview-track { position:relative; min-height:100%; }
    .sc-output-overview-window { position:absolute; display:grid; }
    .sc-output-overview-cell { position:relative; margin:0; min-width:0; min-height:0; display:flex;
      padding:0; border-radius:0; border:0; color:inherit; background:transparent; cursor:pointer; }
    .sc-output-overview-cell:focus-visible { outline:2px solid var(--p-primary-color,var(--ring,#df258e)); outline-offset:-2px; }
    .sc-output-overview-cell-status { position:absolute; inset:0; display:grid; place-items:center; font-size:12px; }
    .sc-output-overview-cell-status:empty { display:none; }
    .sc-output-overview-image { min-height:0; width:100%; height:100%; display:flex; align-items:center; justify-content:center; }
    .sc-output-overview-thumbnail { width:100%; height:100%; object-fit:contain; }
    .sc-output-media-detail { position:absolute; inset:0; background:var(--sugarcubes-canvas-background,#151515); cursor:default; }
    .sc-output-media-detail[hidden] { display:none; }
    .sc-output-media-detail > video,.sc-output-media-detail > img { width:100%; height:100%; object-fit:contain; }
    .sc-output-model-container { position:absolute; inset:0; overflow:hidden; }
    .sc-output-model-container > canvas { position:absolute; inset:0; max-width:100%; max-height:100%; }
    .sc-output-poster-capture { position:fixed; left:-10000px; top:0; width:384px; height:384px; pointer-events:none; }
    .sc-output-media-badge { position:absolute; left:10px; bottom:10px; padding:3px 8px;
      border-radius:6px; background:#000b; color:white; font-size:12px; pointer-events:none; }
  `;
    document.head.append(style);
}
