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
 * The settings workspace's structure, measured from the Python application: navigation pane 280
 * wide with 260×40 items and a sliding 3×16 accent rail; a centred column of at most 1000;
 * section spacing 20/30/6/4; cards with padding 16 and height at least 68; switch, combo, field
 * and button geometry. Every colour and corner comes from the active style's settings look
 * (`SettingsLooks`), so each visual style presents the same organization in its own skin.
 */
import { segmentedSliderCss } from '../../core/SegmentedSlider.js';
import { settingsLookCss } from './SettingsLooks.js';

/** The workspace's stylesheet: every style's look, then the structure. */
export const settingsWorkspaceCss = `
  ${settingsLookCss()}
  .sc-sw { color:var(--sc-sw-text); font:14px/20px var(--sc-font-family,"Segoe UI",system-ui,sans-serif);
    box-sizing:border-box; }
  .sc-sw *, .sc-sw *::before, .sc-sw *::after { box-sizing:border-box; }

  /* The frame: the workspace covers ComfyUI's dialog; ComfyUI's own panels show through beneath
     the toolbar and navigation when one is chosen. */
  [role="dialog"]:has(> [data-sc-sw-host]) { width:min(1400px,94vw) !important; max-width:none !important;
    height:min(900px,90vh) !important; }
  [data-sc-sw-host]:not([data-sc-sw-panel]) > :not(.sc-sw) { visibility:hidden; }
  [data-sc-sw-host][data-sc-sw-panel] > .grid { padding-top:44px; box-sizing:border-box;
    grid-template-columns:280px minmax(0,1fr) !important; }
  [data-sc-sw-host][data-sc-sw-panel] > .grid > nav { visibility:hidden; }
  /* The workspace toolbar closes the dialog; the panel header's own close button would be a second. */
  [data-sc-sw-host][data-sc-sw-panel] > .grid > div > header > button:last-child { display:none; }

  /* A plain ComfyUI page: SugarCubes' category holds one row, the workspace, at full width. */
  .setting-group:has([data-setting-id="SugarCubes.Settings"]) > h3,
  [data-setting-id="SugarCubes.Settings"] .form-label { display:none; }
  [data-setting-id="SugarCubes.Settings"] .form-input { flex:1 1 auto; justify-content:stretch; min-width:0; }
  [data-setting-id="SugarCubes.Settings"] .form-input > * { flex:1 1 auto; min-width:0; }

  /* Frame: the whole dialog is the workspace. */
  .sc-sw[data-mode="frame"] { position:absolute; inset:0; z-index:2; display:grid;
    grid-template-columns:280px minmax(0,1fr); grid-template-rows:44px minmax(0,1fr);
    background:var(--sc-sw-page); pointer-events:auto; }
  .sc-sw[data-mode="frame"][data-panel] { background:transparent; pointer-events:none; }
  .sc-sw[data-mode="frame"][data-panel] :is(.sc-sw-toolbar,.sc-sw-nav,.sc-sw-panel-header) { pointer-events:auto; }
  .sc-sw[data-mode="frame"][data-panel] .sc-sw-nav,.sc-sw[data-mode="frame"][data-panel] .sc-sw-toolbar { background:var(--sc-sw-page); }
  .sc-sw-toolbar { grid-column:1 / -1; display:grid; grid-template-columns:1fr 420px 1fr; align-items:center;
    padding:4px 8px; gap:4px; }
  .sc-sw-search { grid-column:2; position:relative; height:36px; }
  .sc-sw-search input { width:100%; height:36px; padding:0 36px 0 10px; }
  .sc-sw-search .sc-sw-search-icon { position:absolute; right:12px; top:11px; pointer-events:none; opacity:.8; }
  .sc-sw-close { grid-column:3; justify-self:end; width:36px; height:36px; display:grid; place-items:center;
    border:0; border-radius:5px; background:transparent; color:inherit; cursor:pointer; }
  .sc-sw-close:hover { background:var(--sc-sw-hover); }
  .sc-sw-nav { position:relative; display:flex; flex-direction:column; gap:4px; padding-top:12px; overflow-y:auto;
    scrollbar-width:none; }
  .sc-sw-nav-group { margin:14px 0 2px 26px; font-size:12px; font-weight:600; opacity:.7; }
  .sc-sw-nav-item { position:relative; flex:none; width:260px; height:40px; margin:0 10px; display:flex;
    align-items:center; gap:14px; padding:0 14px 0 16px; border:0; border-radius:5px; background:transparent;
    color:inherit; font:inherit; text-align:left; cursor:pointer; }
  .sc-sw-nav-item > span:last-child { min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sc-sw-nav-item::after { content:""; position:absolute; inset:1px; border-radius:5px; pointer-events:none; }
  .sc-sw-nav-item[aria-current="page"] { font-weight:600; }
  .sc-sw-nav-item[aria-current="page"]::after { background:var(--sc-sw-nav-selected); }
  .sc-sw-nav-item:hover::after { background:var(--sc-sw-hover); transition:background 83ms; }
  .sc-sw-nav-item:active::after { background:var(--sc-sw-press); transition:none; }
  .sc-sw-nav-icon { width:18px; height:18px; flex:none; display:grid; place-items:center; }
  .sc-sw-nav-icon > * { max-width:18px; max-height:18px; }
  .sc-sw-rail { position:absolute; left:12px; top:0; width:3px; height:16px; border-radius:1.5px;
    background:var(--sc-sw-accent); pointer-events:none; }
  .sc-sw-rail[hidden] { display:none; }
  .sc-sw-main { min-height:0; overflow-y:auto; scrollbar-width:thin; }
  .sc-sw[data-mode="frame"][data-panel] .sc-sw-main { overflow:hidden; pointer-events:none; }
  .sc-sw-column { width:min(1000px,100%); margin:0 auto; padding:24px; }
  .sc-sw[data-mode="frame"] .sc-sw-column { width:min(1048px,100%); }
  .sc-sw-header { margin:0 0 20px; font-size:28px; line-height:36px; font-weight:600; }
  .sc-sw[data-mode="frame"][data-panel] .sc-sw-column { display:none; }
  .sc-sw-page[data-entering] { animation:sc-sw-enter 180ms cubic-bezier(.65,0,.35,1); }
  @keyframes sc-sw-enter { from { opacity:0; transform:translateX(var(--sc-sw-enter-from,24px)); } }

  /* Browser: one SugarCubes page inside ComfyUI's dialog, its pages as segmented tabs. */
  .sc-sw[data-mode="page"] { width:100%; }
  .sc-sw[data-mode="page"] .sc-sw-page-bar { display:flex; align-items:center; gap:12px; flex-wrap:wrap; margin:0 0 20px; }
  .sc-sw-tabs-strip { display:inline-flex; padding:4px; border:1px solid var(--interface-stroke,#454545); border-radius:8px;
    background:var(--sugarcubes-card-surface,var(--comfy-menu-bg,#202020)); max-width:100%; }
  ${segmentedSliderCss('.sc-segmented-slider', '.sc-segmented-slider-indicator')}
  .sc-sw[data-mode="page"] .sc-sw-search { width:min(280px,100%); margin-left:auto; }

  /* Sections and cards. */
  .sc-sw-section[hidden] { display:none; }
  .sc-sw-section:not([hidden]) ~ .sc-sw-section:not([hidden]) { margin-top:30px; }
  .sc-sw-section-title { margin:0 0 6px; font-size:14px; line-height:20px; font-weight:600; }
  .sc-sw-section-subtitle { margin:-4px 0 6px; font-size:12px; line-height:16px; }
  .sc-sw-cards { display:flex; flex-direction:column; gap:4px; }
  .sc-sw-crumb { margin-top:8px; }
  .sc-sw-card { position:relative; min-width:148px; border:1px solid var(--sc-sw-border);
    border-radius:var(--sc-sw-radius); background:var(--sc-sw-card); }
  .sc-sw-card[hidden], .sc-sw-row[hidden] { display:none; }
  .sc-sw-body-card { border:0; background:transparent; }
  .sc-sw-card-head, .sc-sw-row { position:relative; display:flex; align-items:center; gap:0; min-height:68px; padding:16px; }
  .sc-sw-row { min-height:52px; padding:8px 44px 8px 58px; border-top:1px solid var(--sc-sw-border); }
  .sc-sw-card-icon { width:20px; height:20px; flex:none; margin-right:20px; display:grid; place-items:center; }
  .sc-sw-card-icon > * { max-width:20px; max-height:20px; }
  .sc-sw-card-text { flex:1; min-width:0; display:flex; flex-direction:column; gap:1px; }
  .sc-sw-card-title { font-size:14px; line-height:20px; font-weight:600; overflow-wrap:anywhere; }
  .sc-sw-card-description { font-size:12px; line-height:16px; font-weight:400; overflow-wrap:anywhere; }
  .sc-sw-card-controls { flex:none; display:flex; align-items:center; gap:8px; min-width:120px; margin-left:24px;
    justify-content:flex-end; flex-wrap:nowrap; }
  .sc-sw-card-controls[hidden] { display:none; }
  .sc-sw-card[data-wrapped] .sc-sw-card-head, .sc-sw-row[data-wrapped] { flex-wrap:wrap; row-gap:8px; }
  .sc-sw-card[data-wrapped] .sc-sw-card-controls, .sc-sw-row[data-wrapped] .sc-sw-card-controls {
    flex:1 0 100%; margin-left:40px; flex-direction:column; align-items:flex-start; }
  .sc-sw-row[data-wrapped] .sc-sw-card-controls { margin-left:0; }
  .sc-sw-card[data-narrow] .sc-sw-card-icon { display:none; }
  .sc-sw-card[data-narrow] .sc-sw-card-controls { margin-left:0; }
  .sc-sw-interactive > .sc-sw-card-head { cursor:pointer; }
  .sc-sw-interactive > .sc-sw-card-head::after { content:""; position:absolute; inset:1px; border-radius:5px;
    pointer-events:none; transition:background 83ms; }
  .sc-sw-interactive > .sc-sw-card-head:hover::after { background:var(--sc-sw-hover); }
  .sc-sw-interactive > .sc-sw-card-head:active::after { background:var(--sc-sw-press); transition:none; }
  .sc-sw-expander > .sc-sw-card-head { padding-right:4px; }
  .sc-sw-expander:not(.sc-sw-interactive) > .sc-sw-card-head, .sc-sw-expander:has(> .sc-sw-card-head [role="switch"]) > .sc-sw-card-head { padding-right:16px; }
  .sc-sw-card-body { display:none; }
  .sc-sw-expander[data-open] > .sc-sw-card-body { display:block; animation:sc-sw-expand 333ms cubic-bezier(0,0,0,1); }
  .sc-sw-expander[data-open] > .sc-sw-card-head::after { border-radius:5px 5px 0 0; }
  @keyframes sc-sw-expand { from { opacity:0; transform:translateY(-8px); } }
  .sc-sw-chevron { width:32px; height:32px; flex:none; margin-left:8px; display:grid; place-items:center;
    transition:transform 167ms cubic-bezier(1,1,0,1); }
  .sc-sw-expander[data-open] .sc-sw-chevron { transform:rotate(180deg); transition-duration:333ms;
    transition-timing-function:cubic-bezier(0,0,0,1); }
  @media (prefers-reduced-motion:reduce) {
    .sc-sw-page[data-entering], .sc-sw-expander[data-open] > .sc-sw-card-body { animation:none; }
    .sc-sw-chevron { transition:none; }
  }

  /* Controls (Fluent metrics). */
  .sc-sw-switch { display:inline-flex; align-items:center; justify-content:flex-end; gap:12px; min-width:120px;
    height:22px; padding:0 0 0 2px; border:0; background:transparent; color:inherit; font:inherit; cursor:pointer; }
  .sc-sw-switch-track { position:relative; width:42px; height:22px; flex:none; border-radius:11px;
    border:1px solid var(--sc-sw-switch-off); background:transparent; transition:background 120ms; }
  .sc-sw-switch-track::after { content:""; position:absolute; left:4px; top:4px; width:12px; height:12px;
    border-radius:6px; background:var(--sc-sw-switch-knob-off); transition:left 120ms; }
  .sc-sw-switch[aria-checked="true"] .sc-sw-switch-track { background:var(--sc-sw-accent); border-color:var(--sc-sw-accent); }
  .sc-sw-switch[aria-checked="true"] .sc-sw-switch-track::after { left:24px; background:var(--sc-sw-switch-knob-on); }
  .sc-sw-switch:hover .sc-sw-switch-track { background:var(--sc-sw-control-hover); }
  .sc-sw-switch[aria-checked="true"]:hover .sc-sw-switch-track { background:color-mix(in srgb,var(--sc-sw-accent) 90%,#fff); }
  .sc-sw-switch:disabled { cursor:default; color:var(--sc-sw-disabled); }
  .sc-sw-switch:disabled .sc-sw-switch-track { border-color:var(--sc-sw-disabled); }
  .sc-sw-switch:disabled .sc-sw-switch-track::after { background:var(--sc-sw-disabled); }
  .sc-sw-switch[aria-checked="true"]:disabled .sc-sw-switch-track { background:var(--sc-sw-disabled); border-color:transparent; }
  .sc-sw-field, .sc-sw-button { height:33px; min-width:120px; max-width:100%; border-radius:var(--sc-sw-control-radius); font:inherit; font-size:14px;
    color:inherit; background:var(--sc-sw-control); border:1px solid var(--sc-sw-control-border); }
  .sc-sw-field { padding:0 10px; border-bottom-color:var(--sc-sw-field-edge); outline:none; }
  .sc-sw-field:hover { background:var(--sc-sw-control-hover); }
  .sc-sw-field:focus { background:var(--sc-sw-field-focus); border-bottom:2px solid var(--sc-sw-accent); }
  .sc-sw-field[data-sc-sw-readonly] { opacity:.85; }
  .sc-sw-combo { padding:0 31px 0 11px; appearance:none; cursor:pointer; border-bottom-color:var(--sc-sw-control-border);
    background-image:linear-gradient(45deg,transparent 50%,currentColor 50%),linear-gradient(135deg,currentColor 50%,transparent 50%);
    background-position:calc(100% - 17px) 15px,calc(100% - 12px) 15px; background-size:5px 5px; background-repeat:no-repeat; }
  .sc-sw-combo option { color:CanvasText; background:Canvas; }
  .sc-sw-button { height:32px; padding:0 12px; border-top-color:var(--sc-sw-control-edge); cursor:pointer; white-space:nowrap; }
  .sc-sw[data-scheme="light"] .sc-sw-button { border-top-color:var(--sc-sw-control-border); border-bottom-color:var(--sc-sw-control-edge); }
  .sc-sw-button:hover:not(:disabled) { background:var(--sc-sw-control-hover); }
  .sc-sw-button:active:not(:disabled) { background:var(--sc-sw-control-press); }
  .sc-sw-primary { background:var(--sc-sw-accent); border-color:var(--sc-sw-accent); color:var(--sc-sw-switch-knob-on); }
  .sc-sw-field:disabled, .sc-sw-button:disabled { color:var(--sc-sw-disabled); cursor:default; }
  /* The Accent color row, as Python draws it: 28 px swatch, 72 px hex value, 90 px Choose. */
  .sc-accent-preview, .sc-accent-actions { display:inline-flex; align-items:center; gap:8px; }
  .sc-accent-swatch { width:28px; height:28px; flex:none; box-sizing:border-box; border-radius:5px;
    border:1px solid rgba(128,128,128,.47); }
  .sc-accent-hex { width:72px; font-size:14px; line-height:20px; font-variant-numeric:tabular-nums; }
  .sc-accent-button { width:90px; min-width:90px; }
  .sc-accent-picker { position:absolute; width:0; height:0; padding:0; border:0; opacity:0; pointer-events:none; }
  .sc-sw-status { font-size:14px; line-height:20px; text-align:right; }
  /* The About and Updates pages: Python's section rhythm and setting cards. */
  .sc-sw .sc-settings-about { width:100%; }
  .sc-sw .sc-settings-about h1 { margin:0 0 4px; font-size:20px; line-height:28px; font-weight:600; }
  .sc-sw .sc-settings-about h2 { margin:0 0 6px; font-size:14px; line-height:20px; font-weight:600; }
  .sc-sw .sc-settings-about p { margin:0 0 4px; font-size:14px; line-height:20px; }
  .sc-sw .sc-settings-about__section { margin-top:30px; }
  .sc-sw .sc-settings-about__section > .sc-sw-card + .sc-sw-card { margin-top:4px; }
  .sc-sw .sc-settings-about__identity { display:flex; flex-direction:column; align-items:center; gap:8px; margin-top:20px;
    text-align:center; }
  .sc-sw .sc-settings-about__logo { width:128px; height:128px; }
  .sc-sw .sc-settings-about__identity strong { font-size:14px; line-height:20px; font-weight:600; }
  .sc-sw .sc-settings-about__identity p { margin:0; font-size:12px; line-height:16px; }
  /* Python's grid: two equal columns once the group is 920 px wide, otherwise one. */
  .sc-sw .sc-settings-about__versions { display:grid; gap:4px;
    grid-template-columns:repeat(auto-fill,minmax(min(100%,max(458px,calc((100% - 4px) / 2))),1fr)); }
  .sc-sw .sc-settings-about a.sc-settings-about__version { position:relative; display:flex; align-items:center; gap:10px; min-height:80px;
    padding:0 16px; box-sizing:border-box; border:1px solid var(--sc-sw-border); border-radius:var(--sc-sw-radius);
    background:var(--sc-sw-card); color:inherit; text-decoration:none; cursor:pointer; min-width:0; }
  .sc-sw .sc-settings-about a.sc-settings-about__version::after { content:""; position:absolute; inset:1px; border-radius:5px; pointer-events:none; }
  .sc-sw .sc-settings-about a.sc-settings-about__version:hover::after { background:var(--sc-sw-hover); }
  .sc-sw .sc-settings-about a.sc-settings-about__version:active::after { background:var(--sc-sw-press); }
  .sc-settings-about__version-text { flex:1; min-width:0; display:flex; flex-direction:column; gap:1px; }
  .sc-settings-about__version-text strong { font-size:14px; line-height:20px; font-weight:600; overflow:hidden;
    white-space:nowrap; text-overflow:ellipsis; }
  .sc-settings-about__version-text small { font-size:11px; line-height:14px; display:-webkit-box; -webkit-line-clamp:2;
    -webkit-box-orient:vertical; overflow:hidden; }
  .sc-settings-about__version-meta { flex:none; width:180px; display:flex; flex-direction:column; align-items:flex-end;
    gap:1px; text-align:right; }
  .sc-settings-about__version-meta span { font-size:14px; line-height:20px; user-select:text; max-width:100%;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sc-settings-about__version-meta small { font-size:12px; line-height:16px; max-width:100%; overflow:hidden;
    white-space:nowrap; text-overflow:ellipsis; }
  .sc-sw .sc-settings-about__card[data-stacked] .sc-sw-card-head { flex-wrap:wrap; row-gap:8px; }
  .sc-sw .sc-settings-about__card[data-stacked] .sc-sw-card-controls { flex-basis:100%; justify-content:flex-end; margin-left:0; }
  .sc-sw .sc-settings-about__card .sc-sw-card-description:empty { display:none; }
  .sc-sw .sc-settings-about__card .sc-sw-button { display:inline-flex; align-items:center; gap:8px; min-width:0; }
  .sc-sw .sc-settings-updates .sc-settings-about__card { display:flex; align-items:center; gap:16px; min-height:80px; padding:16px;
    border:1px solid var(--sc-sw-border); border-radius:var(--sc-sw-radius); background:var(--sc-sw-card); min-width:0; }
  .sc-sw .sc-settings-updates .sc-settings-about__card > div { flex:1; min-width:0; }
  .sc-sw .sc-settings-updates .sc-settings-about__card strong { font-weight:600; }
  .sc-sw .sc-settings-updates .sc-settings-about__card small { display:block; margin-top:1px; font-size:12px; line-height:16px; }
  .sc-sw .sc-settings-about a { color:var(--sc-sw-accent); text-decoration:underline; text-underline-offset:3px; }
  .sc-sw .sc-settings-about__actions { display:flex; gap:8px; flex-wrap:wrap; margin-top:8px; }
  .sc-sw .sc-settings-about button { height:32px; padding:0 12px; border-radius:var(--sc-sw-control-radius); font:inherit;
    color:inherit; background:var(--sc-sw-control); border:1px solid var(--sc-sw-control-border); cursor:pointer; }
  .sc-sw .sc-settings-about button:hover:not(:disabled) { background:var(--sc-sw-control-hover); }
  .sc-sw .sc-settings-about button:disabled { color:var(--sc-sw-disabled); cursor:default; }
  .sc-sw .sc-settings-updates__status { font-weight:600; }
  .sc-sw .sc-settings-updates__causes { margin:4px 0; padding-left:20px; line-height:20px; }
  .sc-sw .sc-settings-updates__causes:empty { display:none; }
  .sc-sw-empty { padding:16px; border:1px solid var(--sc-sw-border); border-radius:var(--sc-sw-radius); background:var(--sc-sw-card); }`;
