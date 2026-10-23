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
/** Paint Plugin Mode's existing toolbar and remove orb-dependent frame geometry. */
export const pluginFrameCss = `
  :root[data-sc-plugin-mode] { --sc-generation-top-radius:6px; }
  .sc-substitute-frame[data-plugin-mode] { padding-left:0; }
  .sc-substitute-frame[data-plugin-mode][data-generation-strip] { padding-right:var(--sc-native-caption-width,0px); }
  :root[data-sc-plugin-mode] [data-sc-substitute-toolbar-line] { padding-left:0; }
  [data-sc-plugin-toolbar] { flex-wrap:wrap; align-items:center; justify-content:flex-start; gap:4px; }
  [data-sc-plugin-toolbar] > .sc-overrides-dock-slot { order:20; flex:0 0 100% !important; }
  .sc-plugin-toolbar-start { display:flex; align-items:center; align-self:center; gap:4px; order:-10; flex:none; }
  .sc-plugin-toolbar-start .sc-plugin-settings { display:flex; align-items:center; justify-content:center; position:relative; box-sizing:border-box; width:32px; height:32px; min-width:32px; padding:0; flex:none; }
  .sc-plugin-toolbar-start .sc-application-gear { display:block; width:16px; height:16px; background:currentColor; mask:var(--sc-application-glyph) center/contain no-repeat; }
  .sc-application-gear[hidden] { display:none; }
  [data-sc-plugin-toolbar] > .sc-generation-strip { position:relative; top:auto; right:auto; margin-left:0; flex:none; order:10; align-self:center; align-items:center; }
  [data-sc-plugin-toolbar] > :has(button > i:is([class~="icon-[lucide--panel-right]"], [class~="icon-[lucide--panel-left]"])) { order:11; }
`;
