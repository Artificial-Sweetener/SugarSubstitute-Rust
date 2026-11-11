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
/** Style Fluent editor navigation within native and Substitute toolbar allocations. */
export const workflowEditorNavigationCss = `
      [data-sc-editor-native] { display:none!important; }
      .sc-editor-navigation { display:flex; align-items:center; flex:1 1 auto; min-width:0; height:36px; pointer-events:auto; color:var(--sc-text); }
      .sc-editor-navigation ol { display:flex; align-items:center; gap:4px; overflow-x:auto; white-space:nowrap; list-style:none; margin:0; padding:0; scrollbar-width:thin; }
      .sc-editor-navigation li { display:flex; align-items:center; gap:4px; flex:none; }
      .sc-editor-navigation li + li::before { content:'/'; color:var(--sc-text-muted); }
      .sc-editor-navigation [data-sc-button] { min-width:0; height:32px; }
      .sc-editor-navigation [aria-current] { padding:0 8px; font-weight:var(--sc-font-weight-strong); }
      .sc-editor-navigation .sc-editor-bar-back { flex:none; width:32px; }
      :root[data-sc-substitute-frame] .sc-overrides-dock-slot:has(> [hidden]) { flex:0 0 0; }
    `;
