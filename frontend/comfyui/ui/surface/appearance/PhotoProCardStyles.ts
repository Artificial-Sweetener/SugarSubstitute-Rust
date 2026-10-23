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
/** Style PhotoPro card headings and rows without replacing native collapse controls. */

/** Place the host chevron at the icon edge and retain only card boundary rules. */
export function photoProCardCss(): string {
  return `
    html[data-sc-visual-style="photopro"] .sc-overrides-toolbar[data-presentation="plugin"] > .sc-overrides-card-title { padding:5px 0 5px 10px; font-size:10px; line-height:14px; font-weight:600; }
    html[data-sc-visual-style="photopro"] .sc-overrides-toolbar[data-presentation="plugin"] > .sc-overrides-menu { justify-self:end; box-sizing:border-box; min-width:0; min-height:0; margin:0 10px 0 8px; padding:0; gap:0; border:0; background:transparent; }
    html[data-sc-visual-style="photopro"] .sc-overrides-toolbar[data-presentation="plugin"] > .sc-overrides-menu > :first-child { width:100%; height:100%; }
    html[data-sc-visual-style="photopro"] .sc-overrides-toolbar[data-presentation="plugin"] > .sc-overrides-menu > :last-child { display:none; }
    html[data-sc-visual-style="photopro"] .sugarcubes-workflow-masonry__section { isolation:isolate; }
    html[data-sc-visual-style="photopro"] .sugarcubes-workflow-masonry__section::before {
      content:''; position:absolute; z-index:-1; pointer-events:none; top:36px; left:0; width:100%;
      height:calc(var(--sc-masonry-section-height,36px) - 36px); background:var(--sc-surface);
      box-shadow:inset 0 -1px 0 var(--sc-border);
    }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card [data-sc-style-node] { --color-bypass:var(--sc-text-secondary); }
    html[data-sc-visual-style="photopro"] [data-sc-titlebar-controls] [data-cube-card-activation] { margin:0 0 0 8px; padding:0; }
    html[data-sc-visual-style="photopro"] [data-sc-titlebar-controls] > :first-child { justify-content:flex-start; gap:0; }
    html[data-sc-visual-style="photopro"] [data-sc-titlebar-controls] > :first-child > :has(> [data-testid="node-title"]) { flex:0 1 auto; margin-right:0; }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card .sugarcubes-workflow-masonry__node-icon { display:none; }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card [data-masonry-title-icon] { padding-left:0; }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card [data-sc-style-node] [data-testid^="node-header-"] {
      height:24px; min-height:24px; padding:0 2px; border-bottom:0;
    }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card [data-sc-titlebar-controls]:has([data-testid="node-collapse-button"]:not([hidden])) > :first-child {
      padding-left:17px; padding-right:0;
    }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card [data-sc-style-node] [data-testid="node-title"] {
      padding-left:0; font-size:10px; line-height:14px; font-weight:600;
    }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card [data-sc-style-node] [data-testid="node-title"] .editable-text { font-weight:600; }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card [data-testid="node-collapse-button"] {
      left:0; right:auto; width:14px; height:18px; padding:0; color:var(--sc-text-subtle); background:transparent; border:0;
    }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card [data-testid="node-collapse-button"] i { width:11px; height:11px; rotate:0deg; transform:rotate(0deg) !important; }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card [data-sc-titlebar-collapsed="true"] [data-testid="node-collapse-button"] i { transform:rotate(-90deg) !important; }
    html[data-sc-visual-style="photopro"] :is([data-sc-style-node], .sc-overrides-toolbar[data-presentation="plugin"]) [data-sc-style-row],
    html[data-sc-visual-style="photopro"] :is([data-sc-style-node], .sc-overrides-toolbar[data-presentation="plugin"]) [data-sc-style-row] ~ [data-sc-style-row],
    html[data-sc-visual-style="photopro"] [data-sc-style-node] [data-sc-style-ports]:not([hidden]) { border-top:0 !important; border-bottom:0 !important; }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card [data-sc-style-node] [data-testid="advanced-inputs-button"] {
      display:flex; align-items:center; justify-content:center; flex:none; box-sizing:border-box;
      width:28px; min-width:28px; height:20px; min-height:20px; padding:0; margin:4px 8px 6px auto;
      border:1px solid var(--sc-button-bottom-stroke); border-radius:2px;
      background:var(--sc-button-fill); color:var(--sc-text-secondary); font-size:0; gap:0;
    }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card [data-testid="advanced-inputs-button"] > * { display:none; }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card [data-testid="advanced-inputs-button"]::after { content:'…'; font:600 16px/1 var(--sc-font-family); }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card [data-sc-style-row][data-sc-style-column="2"]::before { display:none; }
    html[data-sc-visual-style="photopro"] .sugarcubes-cube-face__node-card [data-sc-style-node] > :has(> [data-testid="advanced-inputs-button"]) { margin-top:0; z-index:auto; background:var(--sc-card-fill); }
  `;
}
