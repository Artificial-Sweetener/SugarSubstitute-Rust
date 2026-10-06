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
/** Style relation chrome with Comfy tokens and explicit embedded Substitute presentation. */
export function ensureRelationStyles(document: Document): void {
  if (document.getElementById('sc-relation-styles')) return;
  const style = document.createElement('style');
  style.id = 'sc-relation-styles';
  style.textContent = `
  .sc-overrides-toolbar { display:flex; flex:none; align-items:center; gap:4px; margin:0 4px 4px; padding:4px 8px; border:1px solid var(--interface-stroke); border-radius:var(--radius-lg); background:var(--comfy-menu-bg); color:var(--fg-color); font:inherit; font-size:var(--text-xs); pointer-events:auto; min-width:0; overflow-x:auto; }
  .sc-overrides-toolbar[hidden] { display:none; }
  .sc-override-control { --sc-override-width:180px; flex:0 1 auto; min-width:calc(var(--sc-override-label-width,0px) + 64px); }
  .sc-override-control[data-semantic="seed"] { --sc-override-width:192px; }
  .sc-override-control[data-semantic="steps"], .sc-override-control[data-semantic="cfg"] { --sc-override-width:80px; }
  .sc-override-control > div { display:block; width:auto; }
  .sc-override-control :has(> [data-testid="widget-layout-field-label"]) { display:flex; align-items:center; gap:4px; width:auto; }
  .sc-override-control [data-testid="widget-layout-field-label"] { flex:none; width:auto; padding:0 4px; white-space:nowrap; }
  .sc-override-control :has(> [data-testid="widget-layout-field-label"]) > :last-child { flex:0 1 var(--sc-override-width); width:var(--sc-override-width); max-width:var(--sc-override-width); min-width:60px; }
  .sc-override-control :has(> [data-testid="widget-layout-field-label"]) > :last-child > :first-child { min-width:0; max-width:100%; }
  [data-sc-choice-locked] { color:var(--muted-foreground); opacity:.5; cursor:default; }
  [data-sc-choice-lock] { margin-left:auto; flex-shrink:0; }
  .sc-field-relation.sc-node-link { margin-left:auto; flex:0 1 var(--sc-link-preferred-width,160px); width:var(--sc-link-preferred-width,160px); min-width:60px; max-width:var(--sc-link-preferred-width,160px); height:33px; }
  .lg-node-header:has(.sc-node-link) > :first-child > :first-child:has([data-testid="node-title"]) { flex-shrink:0; }
  .sc-field-relation.sc-node-link.sc-relation-canvas-header { max-width:50%; }
  [data-sc-relation-hidden="true"] { display:none !important; }
  .sc-field-relation { display:flex; align-items:center; min-width:0; flex:1; color:var(--fg-color); font:inherit; }
  [data-sc-choice-inline] [data-testid="widget-layout-field-label"] { display:none; }
  [data-sc-choice-inline] :has(> [data-testid="widget-layout-field-label"]) { display:block; }
  .sugarcubes-native-widget[data-sc-choice-inline] > [data-sc-fluent-control] { width:100%; min-width:0; grid-column:auto; }
  .sugarcubes-native-widget[data-sc-choice-inline] [data-sc-fluent-layout] { display:block; width:100%; min-height:0; grid-column:auto; }
  .sugarcubes-native-widget[data-sc-choice-inline] [data-sc-fluent-layout] > :last-child { width:100%; max-width:none; min-width:0; flex:1; }
  .sc-overrides-menu { flex:none; display:flex; flex-direction:row; align-items:center; justify-content:center; gap:4px; height:32px; width:40px; padding:6px; border:0; border-radius:var(--radius-md); color:var(--fg-color); background:transparent; cursor:pointer; }
  .sc-overrides-menu:hover, .sc-overrides-menu[aria-expanded="true"] { background:var(--comfy-input-bg); }
  .sc-overrides-menu:focus-visible { outline:2px solid var(--component-node-widget-background-highlighted); outline-offset:-2px; }
  .sc-overrides-menu > :first-child { width:16px; height:16px; }
  .sc-overrides-menu > :last-child { width:8px; height:8px; }
  .sc-overrides-toolbar[data-presentation="substitute"] > .sc-overrides-menu { flex-direction:column; gap:1px; height:33px; width:24px; padding:2px; }
  .sc-overrides-toolbar[data-presentation="substitute"] > .sc-overrides-menu > :first-child { width:14px; height:14px; }
  [data-sc-link-display] { font-size:0; }
  [data-sc-link-display]::after { content:attr(data-sc-link-display) / ""; font:var(--sc-link-font); display:block; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sc-relation-canvas-header { position:absolute; top:2px; right:8px; z-index:2; }
  .sc-relation-canvas-header, .sc-relation-canvas-field { pointer-events:auto; }
  .sc-relation-canvas-field { position:absolute; z-index:2; }
  `;
  document.head.append(style);
}
