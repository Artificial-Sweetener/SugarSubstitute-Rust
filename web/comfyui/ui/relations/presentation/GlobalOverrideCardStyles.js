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
/** Fill the available card area with responsive rows of existing override controls. */
export const globalOverrideCardCss = `
  .sc-overrides-card-title { display:none; }
  .sc-overrides-graph-content { position:relative; width:100%; height:100%; }
  html .sc-overrides-toolbar[data-sc-overrides-graph] { width:100%; border:0; border-radius:0; }
  .sc-overrides-toolbar[data-sc-overrides-graph] > .sc-overrides-card-title { display:none !important; }
  .sc-overrides-toolbar[data-sc-overrides-graph] > .sc-overrides-menu { position:absolute; top:0; right:0; z-index:1; }

  html .sc-overrides-toolbar[data-presentation="plugin"] {
    position:relative; display:grid; grid-template-columns:minmax(0,1fr) auto;
    align-self:stretch; align-items:center; width:100%;
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
    display:flex; grid-column:1 / -1; flex-flow:row wrap; align-items:stretch;
    width:100%; gap:0; margin:0; overflow:visible;
  }
  .sc-overrides-toolbar[data-presentation="plugin"] .sc-override-control {
    flex:1 0 calc(var(--sc-override-label-width,0px) + var(--sc-override-width) + 24px);
    width:auto; max-width:100%; min-width:min(100%,calc(var(--sc-override-label-width,0px) + 88px));
    box-sizing:border-box; padding:8px 10px;
  }
  html[data-sc-compact-masonry] .sc-overrides-toolbar[data-presentation="plugin"] { border-radius:0; border-width:0 1px 1px 0; }
  html[data-sc-compact-masonry] .sc-overrides-toolbar[data-presentation="plugin"] .sc-override-control { padding-top:4px; padding-bottom:4px; }
  .sc-overrides-toolbar[data-presentation="plugin"] .sc-override-control :has(> [data-testid="widget-layout-field-label"]) { width:100%; }
  .sc-overrides-toolbar[data-presentation="plugin"] .sc-override-control [data-testid="widget-layout-field-label"] { flex:none; padding:0; }
  .sc-overrides-toolbar[data-presentation="plugin"] .sc-override-control :has(> [data-testid="widget-layout-field-label"]) > :last-child { margin-left:auto; max-width:65%; }
`;
