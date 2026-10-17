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
import { ensureOwnedControlAppearanceStyles } from '../../surface/appearance/OwnedControlAppearanceStyles.js';

/** Install relation geometry while shared native control foregrounds retain one owner. */
export function ensureRelationStyles(document: Document): void {
  ensureOwnedControlAppearanceStyles(document);
  if (document.getElementById('sc-relation-styles')) return;
  const style = document.createElement('style');
  style.id = 'sc-relation-styles';
  style.textContent = `
  .sc-overrides-toolbar { position:fixed; z-index:1200; display:flex; flex:none; align-self:flex-start; align-items:center; gap:0; min-height:43px; width:max-content; max-width:calc(100% - 8px); box-sizing:border-box; margin:0; padding:4px; border:1px solid var(--interface-stroke); border-radius:var(--radius-lg); background:var(--comfy-menu-bg); color:var(--fg-color); font:inherit; font-size:var(--text-xs); pointer-events:auto; min-width:0; transition:left 200ms cubic-bezier(.1,.9,.2,1),top 200ms cubic-bezier(.1,.9,.2,1); }
  .sc-overrides-toolbar[hidden] { display:none; }
  .sc-overrides-dock-slot { display:flex; flex:none; align-items:center; min-width:0; }
  .actionbar-container > [data-sc-empty-run-content] { display:none!important; }
  .sc-overrides-toolbar[data-placement="docked"] { position:static; align-self:center; min-height:32px; margin:0; padding:0; border:0; background:transparent; transition:none; }
  .sc-overrides-toolbar[data-placement="docked"] > .sc-overrides-disclosure { order:-1; margin-right:4px; }
  .sc-overrides-toolbar[data-placement="docked"][data-collapsed="true"] > .sc-overrides-menu { margin-right:0; }
  .sc-overrides-dock-target { display:flex; flex:none; align-self:stretch; align-items:center; justify-content:center; box-sizing:border-box; margin:6px 0; border:2px dashed var(--primary-background); border-radius:var(--radius-md); color:var(--fg-color); opacity:.8; pointer-events:none; }
  .sc-overrides-dock-target[data-sc-override-drop-target] { border-width:3px; opacity:1; box-shadow:0 0 20px color-mix(in srgb,var(--primary-background) 50%,transparent); }
  .sc-overrides-grip, .sc-overrides-menu { margin-right:4px; }
  .sc-overrides-fields { display:flex; align-items:center; gap:4px; flex:0 1 auto; min-width:0; margin-right:4px; overflow-x:auto; }
  .sc-overrides-fields[hidden] { display:none; }
  .sc-overrides-toolbar[data-sc-override-moving] { overflow:clip; }
  .sc-overrides-fields[data-sc-override-overflow] > .sc-override-control { flex-shrink:0; }
  .sc-overrides-toolbar[data-sc-override-moving] .sc-overrides-fields { overflow:clip; }
  .sc-overrides-toolbar[data-collapsed="true"] .sc-overrides-menu > :last-child { display:none; }
  .sc-overrides-toolbar[data-collapsed="true"] .sc-overrides-fields { margin-right:0; }
  .sc-overrides-toolbar[data-sc-override-dragging] { transition:none; pointer-events:none; user-select:none; z-index:1400; }
  .sc-overrides-toolbar[data-sc-override-dragging] .sc-overrides-grip { cursor:grabbing; }
  .sc-overrides-grip, .sc-overrides-disclosure { flex:none; display:flex; align-items:center; justify-content:center; width:24px; height:32px; padding:4px; border:0; border-radius:var(--radius-md); color:var(--fg-color); background:transparent; cursor:pointer; }
  .sc-overrides-grip { width:12px; padding:0; margin-right:8px; border-radius:0; font:inherit; font-family:var(--sc-native-grip-font-family,var(--font-inter)); cursor:grab; touch-action:none; }
  .sc-overrides-toolbar[data-collapsed="true"] > .sc-overrides-grip { display:none; }
  .sc-overrides-disclosure > span { width:16px; height:16px; }
  .sc-overrides-disclosure:hover { background:var(--comfy-input-bg); }
  .sc-overrides-grip:focus-visible, .sc-overrides-disclosure:focus-visible { outline:2px solid var(--component-node-widget-background-highlighted); outline-offset:-2px; }
  @media (prefers-reduced-motion:reduce) { .sc-overrides-toolbar { transition:none; } }
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
  .sc-field-relation[hidden] { display:none !important; }
  [data-sc-choice-inline] [data-testid="widget-layout-field-label"] { display:none; }
  [data-sc-choice-inline] :has(> [data-testid="widget-layout-field-label"]) { display:block; width:100%; min-width:0; }
  [data-sc-choice-inline] :has(> [data-testid="widget-layout-field-label"]) > :last-child { width:100%; max-width:none; min-width:0; flex:1; }
  .sugarcubes-native-widget[data-sc-choice-inline] > [data-sc-style-control] { width:100%; min-width:0; grid-column:auto; }
  .sugarcubes-native-widget[data-sc-choice-inline] [data-sc-style-layout] { display:block; width:100%; min-height:0; grid-column:auto; }
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
