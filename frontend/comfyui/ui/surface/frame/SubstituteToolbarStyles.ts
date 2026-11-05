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
/** Present the native actionbar as Substitute's fixed override row beside the orb's pin. */
export const substituteToolbarCss = `
  :root[data-sc-substitute-frame] [data-sc-substitute-toolbar-menu] { margin-left:0; padding-top:0; gap:0;
    background:var(--sc-substitute-toolbar-fill,var(--comfy-menu-bg,#202020)); }
  :root[data-sc-substitute-frame] [data-sc-workflow-selector],
  :root[data-sc-substitute-frame] [data-sc-substitute-extensions],
  :root[data-sc-substitute-frame] [data-sc-substitute-workflow-column] { display:none!important; }
  :root[data-sc-substitute-frame] [data-sc-substitute-toolbar-line] { display:block; min-width:0; height:44px; min-height:44px;
    flex-shrink:0; overflow:visible; padding-left:62px; background:var(--sc-substitute-toolbar-fill,var(--comfy-menu-bg,#202020)); }
  /* A translucent fill must cover the row once, so a filled menu owns it for its nested line. */
  :root[data-sc-substitute-frame] [data-sc-substitute-toolbar-menu] > [data-sc-substitute-toolbar-line] { background:transparent; }
  :root[data-sc-substitute-frame] [data-sc-substitute-toolbar-column] { width:100%; margin:0; min-width:0; align-items:stretch; }
  :root[data-sc-substitute-frame] [data-sc-substitute-toolbar-column] > div { min-width:0; width:100%; }
  :root[data-sc-substitute-frame] [data-sc-substitute-toolbar] { height:44px; min-height:44px; min-width:0; width:100%;
    margin:0; padding:4px 6px 4px 0; border:0; border-radius:0; box-shadow:none; background:transparent; box-sizing:border-box; }
  :root[data-sc-substitute-frame] [data-sc-substitute-toolbar] .actionbar { position:static!important; transform:none!important;
    margin:0!important; padding:0!important; border:0!important; background:transparent!important; box-shadow:none!important; }
  :root[data-sc-substitute-frame] [data-sc-substitute-toolbar] .actionbar .drag-handle { display:none!important; }
  :root[data-sc-substitute-frame] [data-sc-substitute-toolbar] .p-panel-content { padding:0!important; }
  :root[data-sc-substitute-frame] [data-sc-substitute-toolbar] > .sc-overrides-dock-slot { flex:1 1 auto; min-width:0; }
  .sc-overrides-toolbar[data-presentation="substitute"] { position:static; min-height:36px; align-self:center;
    width:100%; padding:0; margin:0; border:0; border-radius:0; background:transparent; transition:none; }
  .sc-overrides-toolbar[data-presentation="substitute"] > :is(.sc-overrides-grip,.sc-overrides-menu,.sc-overrides-disclosure) { display:none!important; }
  .sc-overrides-toolbar[data-presentation="substitute"] .sc-overrides-fields { margin-right:0; flex:1 1 auto; }
`;
