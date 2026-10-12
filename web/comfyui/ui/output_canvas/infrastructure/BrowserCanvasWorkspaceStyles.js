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
/** Style owned browser workspace regions while inheriting Fluent and Comfy appearance tokens. */
import { MASONRY_STACK_Z_INDEX } from '../../core/OverlayStacking.js';
/** Install static allocation chrome without changing native toolbar or renderer selectors. */
export function ensureBrowserCanvasWorkspaceStyles(document) {
    if (document.getElementById('sugarcubes-output-workspace-styles'))
        return;
    const style = document.createElement('style');
    style.id = 'sugarcubes-output-workspace-styles';
    style.textContent = `
    .sc-output-canvas-toggle { display:inline-flex; align-items:center; justify-content:center; flex-shrink:0;
      width:32px; height:32px; padding:0; border:0; border-radius:6px; cursor:pointer;
      color:var(--fg-color,#ddd); background:var(--secondary-background,var(--comfy-menu-bg,#202020)); }
    .sc-output-canvas-toggle:hover { background:var(--secondary-background-hover,#404040); }
    .sc-output-canvas-toggle:focus-visible { outline:2px solid var(--ring,currentColor); outline-offset:2px; }
    .sc-output-canvas-toggle:disabled { opacity:.5; cursor:default; }
    .sc-output-canvas-toggle span { width:20px; height:20px; background:currentColor; pointer-events:none;
      mask:var(--sc-output-icon) center / contain no-repeat; }
    .sc-output-workspace-content { position:absolute; inset:0; pointer-events:none; }
    .sc-output-workspace-pane { position:absolute; left:0; top:0; bottom:0; z-index:${MASONRY_STACK_Z_INDEX + 2};
      display:flex; flex-direction:column; box-sizing:border-box; pointer-events:auto;
      color:var(--fg-color,#ddd); background:var(--comfy-menu-bg,#202020);
      border-right:1px solid var(--interface-stroke,#454545); font:13px var(--fontFamilyBase,"Segoe UI",system-ui,sans-serif); }
    .sc-output-workspace-pane[hidden],.sc-output-workspace-backdrop[hidden] { display:none; }
    .sc-output-workspace-close { position:absolute; top:8px; right:14px; z-index:3; opacity:0;
      cursor:pointer; color:inherit; background:var(--comfy-menu-bg,#202020); border:0;
      border-radius:4px; width:28px; height:28px; font-size:20px; }
    .sc-output-workspace-pane:hover .sc-output-workspace-close,.sc-output-workspace-close:focus-visible { opacity:1; }
    .sc-output-workspace-close:hover { background:var(--secondary-background-hover,#444); }
    .sc-output-workspace-view { position:relative; flex:1; min-height:0; margin-right:6px; overflow:hidden; }
    .sc-output-workspace-divider { position:absolute; top:0; bottom:0; right:0; width:6px; cursor:ew-resize;
      touch-action:none; background:var(--interface-stroke,#454545); pointer-events:auto; }
    .sc-output-workspace-divider:hover { background:var(--p-primary-color,var(--ring,#df258e)); }
    .sc-output-workspace-divider:focus-visible,.sc-output-workspace-close:focus-visible {
      outline:2px solid var(--p-primary-color,var(--ring,#df258e)); outline-offset:-2px; }
    .sc-output-workspace-backdrop { position:absolute; inset:0; z-index:${MASONRY_STACK_Z_INDEX + 1};
      border:0; background:#0005; pointer-events:auto; cursor:pointer; }
  `;
    document.head.append(style);
}
