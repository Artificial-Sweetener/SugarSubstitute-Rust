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
/** Lay out the existing override controls as a fixed, stacked node card. */
export const globalOverrideCardCss = `
  [data-sc-plugin-overrides-dock] { height:auto !important; align-items:flex-start; }
  [data-sc-substitute-toolbar-line]:has([data-sc-plugin-overrides-dock]) { height:auto !important; }
  .sc-overrides-card-title { display:none; }
  html .sc-overrides-toolbar[data-presentation="plugin"] {
    position:relative; display:grid; grid-template-columns:minmax(0,1fr) auto;
    align-self:stretch; align-items:center; width:min(100%,calc(var(--sugarcubes-masonry-column-width,400) * 1px));
    max-width:100%; min-height:0; padding:0; margin:0; z-index:auto; transition:none;
    border:1px solid var(--sc-card-stroke,var(--interface-stroke,#454545));
    border-radius:var(--sc-node-card-radius,12px); background:var(--sc-card-fill,var(--comfy-input-bg,#222));
    color:var(--sc-text,var(--fg-color,#ddd));
  }
  html .sc-overrides-toolbar[data-presentation="plugin"][hidden] { display:none; }
  .sc-overrides-toolbar[data-presentation="plugin"] > .sc-overrides-card-title {
    display:block; grid-column:1; grid-row:1; margin:0; padding:10px;
    font:400 14px/1.4 var(--sc-font-family,system-ui);
  }
  .sc-overrides-toolbar[data-presentation="plugin"] > .sc-overrides-menu { grid-column:2; grid-row:1; margin:0 6px 0 0; }
  .sc-overrides-toolbar[data-presentation="plugin"] > :is(.sc-overrides-grip,.sc-overrides-disclosure) { display:none; }
  .sc-overrides-toolbar[data-presentation="plugin"] > .sc-overrides-fields {
    display:flex; grid-column:1 / -1; flex-direction:column; align-items:stretch;
    width:100%; gap:0; margin:0; overflow:visible;
  }
  .sc-overrides-toolbar[data-presentation="plugin"] .sc-override-control { width:100%; min-width:0; box-sizing:border-box; padding:8px 10px; }
  html[data-sc-compact-masonry] .sc-overrides-toolbar[data-presentation="plugin"] { border-radius:0; border-width:0 1px 1px 0; }
  html[data-sc-compact-masonry] .sc-overrides-toolbar[data-presentation="plugin"] .sc-override-control { padding-top:4px; padding-bottom:4px; }
  .sc-overrides-toolbar[data-presentation="plugin"] .sc-override-control :has(> [data-testid="widget-layout-field-label"]) { width:100%; }
  .sc-overrides-toolbar[data-presentation="plugin"] [data-testid="widget-layout-field-label"] { flex:1; padding:0; }
  .sc-overrides-toolbar[data-presentation="plugin"] .sc-override-control :has(> [data-testid="widget-layout-field-label"]) > :last-child { max-width:65%; }
`;
