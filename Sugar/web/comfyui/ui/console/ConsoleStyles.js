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
 * Style the console with the Python application's Comfy Console values: 12/10 px padding, an 8 px
 * gap, a 15 px semibold title, and a near-black monospace surface with a 1 px light border, a
 * brighter bottom edge and 6 px corners. The masonry above it ends where the console begins.
 */
import { MASONRY_SURFACE_Z_INDEX } from '../core/OverlayStacking.js';
const STYLE_ID = 'sugarcubes-console-styles';
/** Install the console stylesheet once per document. */
export function ensureConsoleStyles(document) {
    if (document.getElementById(STYLE_ID))
        return;
    const style = document.createElement('style');
    style.id = STYLE_ID;
    style.textContent = `
    .sc-console {
      position:absolute; left:0; right:0; bottom:0; z-index:${MASONRY_SURFACE_Z_INDEX + 1};
      height:var(--sc-console-height,200px); box-sizing:border-box;
      display:flex; flex-direction:column; gap:8px; padding:10px 12px;
      pointer-events:auto; color:var(--sc-text,rgba(244,247,250,.94));
      background:var(--sugarcubes-masonry-surface,var(--comfy-menu-bg,#202020));
      font-family:var(--sc-font-family,inherit);
    }
    .sc-console__resize {
      position:absolute; left:0; right:0; top:-3px; height:6px; cursor:ns-resize; touch-action:none;
    }
    .sc-console__resize::after {
      content:''; position:absolute; left:0; right:0; top:3px; height:1px;
      background:var(--sc-border-subtle,rgba(255,255,255,.086));
    }
    .sc-console__header { display:flex; align-items:center; gap:4px; flex:none; }
    .sc-console__title {
      flex:1; min-width:0; font-size:15px; font-weight:600;
      color:var(--sc-text,rgba(244,247,250,.94));
    }
    .sc-console__tool {
      display:inline-flex; align-items:center; justify-content:center; width:32px; height:32px;
      padding:0; border:0; border-radius:4px; background:transparent; color:inherit; cursor:pointer;
    }
    .sc-console__tool:hover { background:color-mix(in srgb,currentColor 9%,transparent); }
    .sc-console__tool:focus-visible { outline:2px solid var(--sc-accent-color,#ff4089); outline-offset:-2px; }
    .sc-console__tool svg {
      width:16px; height:16px; fill:none; stroke:currentColor; stroke-width:1.2;
      stroke-linecap:round; stroke-linejoin:round;
    }
    .sc-console__output {
      flex:1; min-height:0; margin:0; overflow:auto; box-sizing:border-box; padding:6px 8px;
      background:rgba(8,10,12,.97); border:1px solid rgba(255,255,255,.22);
      border-bottom-color:rgba(255,255,255,.44); border-radius:6px;
      color:rgba(230,236,241,.92); white-space:pre-wrap; overflow-wrap:anywhere;
      font:12px/1.4 "Cascadia Mono","Cascadia Code",Consolas,Menlo,Monaco,"Liberation Mono",
        "DejaVu Sans Mono","Noto Sans Mono","Courier New",monospace;
      outline:none;
    }
    .sc-console__output::selection, .sc-console__output *::selection {
      background:var(--sc-accent-color,#ff4089); color:rgba(255,255,255,.98);
    }
  `;
    document.head.append(style);
}
