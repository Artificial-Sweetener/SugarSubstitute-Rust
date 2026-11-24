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
 * In the SugarSubstitute frame, ComfyUI's graph toolbar (select mode, fit, zoom, minimap, links)
 * mirrors the output canvas tab bar across the divider: the same 40 px height from 4 px padding
 * around 30 px controls, the same 8 px corners, border, surface and shadow, and the same 8 px inset
 * from its outer edge and from the graph's bottom (`.sc-output-tabstrip`). Outside the frame the
 * toolbar is left exactly as ComfyUI draws it.
 */
/** The tab bar's geometry the graph toolbar mirrors; a contract test pins it to the tab bar. */
export const GRAPH_TOOLBAR_MIRROR = { inset: 8, padding: 4, control: 30, radius: 8 };
const TOOLBAR = ':root[data-sc-substitute-frame] .graph-canvas-panel > .pointer-events-auto > [role="toolbar"][data-pc-name="buttongroup"]';
/** Restyle ComfyUI's graph toolbar only under the SugarSubstitute frame. */
export const graphToolbarCss = (() => {
    const { inset, padding, control, radius } = GRAPH_TOOLBAR_MIRROR;
    return `
  ${TOOLBAR} {
    right:calc(var(--sc-output-workspace-reserved-width,0px) + ${String(inset)}px);
    bottom:calc(var(--sc-output-workspace-reserved-height,0px) + ${String(inset)}px);
    height:${String(control + 2 * padding + 2)}px; padding:${String(padding)}px; gap:4px; box-sizing:border-box;
    align-items:center; border:1px solid var(--interface-stroke,#454545); border-radius:${String(radius)}px;
    background:var(--sugarcubes-card-surface,var(--comfy-menu-bg,#202020)); box-shadow:0 2px 8px #0003; }
  ${TOOLBAR} button { height:${String(control)}px; min-height:${String(control)}px; border-radius:5px; }`;
})();
