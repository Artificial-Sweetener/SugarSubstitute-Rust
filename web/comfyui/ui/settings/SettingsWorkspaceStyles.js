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
/** Match Substitute's settings geometry using official Fluent tokens and native Comfy controls. */
export const settingsWorkspaceCss = `
[data-sc-settings] {
  --sc-settings-text:var(--p-text-color,var(--fg-color));
  --sc-settings-muted:var(--p-text-muted-color,var(--descrip-text));
  --sc-settings-card:var(--p-content-background,var(--comfy-menu-bg));
  --sc-settings-border:var(--p-content-border-color,var(--border-color));
}
[data-sc-settings] [data-sc-setting-kind="owned"] > div {
  align-items:flex-start; flex-wrap:wrap; row-gap:12px;
}
[data-sc-settings] [data-sc-setting-kind="owned"] .form-label { min-width:180px; }
[data-sc-settings] [data-sc-setting-kind="owned"] .form-input {
  min-width:0; max-width:100%; flex:1 1 320px; justify-content:flex-start;
}
[data-sc-settings] [data-sc-setting-kind="owned"] .sugarcubes-settings-row {
  width:100%; min-width:0; gap:8px; flex-wrap:wrap;
}
[data-sc-settings] [data-sc-setting-kind="owned"] button {
  white-space:normal; min-height:32px; padding:6px 12px; border-radius:6px;
  border:1px solid var(--p-content-border-color,var(--border-color));
  color:var(--p-text-color,var(--fg-color));
  background:var(--secondary-background,var(--comfy-input-bg)); font:inherit; cursor:pointer;
}
[data-sc-settings] [data-sc-setting-kind="owned"] button:hover { background:var(--secondary-background-hover,var(--comfy-menu-bg)); }
[data-sc-settings] [data-sc-setting-kind="owned"] button:disabled { opacity:.5; cursor:default; }
[data-sc-settings] .sc-output-saving-setting > span { min-width:min(240px,100%); }
[data-sc-settings] .sugarcubes-settings-row input[type="checkbox"] { width:18px; height:18px; }
[data-sc-settings] .sc-settings-about { width:100%; color:var(--sc-settings-text); }
[data-sc-settings] [data-setting-id="SugarCubes.About.Information"] > div { display:block; }
[data-sc-settings] [data-setting-id="SugarCubes.About.Information"] .form-label { display:none; }
[data-sc-settings] [data-setting-id="SugarCubes.About.Information"] .form-input { display:block; }
.sc-settings-about h1 { font-size:28px; font-weight:600; margin:0 0 8px; }
.sc-settings-about h2 { font-size:18px; font-weight:600; margin:28px 0 12px; }
.sc-settings-about p { margin:8px 0; line-height:1.55; }
.sc-settings-about__identity { padding:12px 0 8px; }
.sc-settings-about__versions { display:grid; grid-template-columns:repeat(auto-fit,minmax(min(100%,320px),1fr)); gap:8px; }
.sc-settings-about__card {
  display:flex; align-items:center; gap:16px; padding:16px;
  border:1px solid var(--sc-settings-border); border-radius:6px;
  background:var(--sc-settings-card); min-width:0;
}
.sc-settings-about__card > div { flex:1; min-width:0; }
.sc-settings-about__card small { display:block; color:var(--sc-settings-muted); line-height:1.5; margin-top:4px; }
.sc-settings-about__card strong { font-weight:600; }
.sc-settings-about__card a { flex-shrink:0; }
.sc-settings-about a { color:var(--sc-accent-indicator,var(--p-primary-color)); text-decoration:underline; text-underline-offset:3px; }
.sc-settings-about__actions { display:flex; gap:16px; flex-wrap:wrap; margin-top:12px; }
.sc-settings-about__status { color:var(--sc-settings-muted); }
.sc-settings-about button { font:inherit; }
[data-sc-settings-fluent] {
  --sc-settings-text:var(--colorNeutralForeground1);
  --sc-settings-muted:var(--colorNeutralForeground2);
  --sc-settings-card:var(--colorNeutralBackground1);
  --sc-settings-border:var(--colorNeutralStroke2);
  --p-primary-color:var(--sc-accent-indicator);
  --p-inputtext-background:var(--colorNeutralBackground1);
  --p-inputtext-color:var(--colorNeutralForeground1);
  --p-inputtext-border-color:var(--colorNeutralStroke1);
  --p-inputtext-focus-border-color:var(--sc-accent-indicator);
  --p-select-background:var(--colorNeutralBackground1);
  --p-select-color:var(--colorNeutralForeground1);
  --p-select-border-color:var(--colorNeutralStroke1);
  --p-select-focus-border-color:var(--sc-accent-indicator);
  --p-slider-range-background:var(--sc-accent-indicator);
  --p-toggleswitch-checked-background:var(--sc-accent-indicator);
  --p-toggleswitch-checked-hover-background:var(--sc-accent-hover);
  font-family:var(--fontFamilyBase); color:var(--colorNeutralForeground1);
  border-radius:8px !important; background:var(--colorNeutralBackground2);
}
[role="dialog"]:has(> [data-sc-settings-fluent]) {
  border-radius:8px; background:var(--sc-settings-card);
  width:min(1400px,94vw); height:min(900px,90vh);
}
[data-sc-settings-fluent] > div {
  height:calc(100% - 64px); grid-template-columns:260px minmax(0,1fr) !important;
}
[data-sc-settings-fluent] > div > nav {
  background:var(--colorNeutralBackground2); padding-top:8px;
}
[data-sc-settings-fluent] > div > nav > header,
[data-sc-settings-fluent] > div > div > header { display:none; }
[data-sc-settings-fluent] > div > div { background:var(--colorNeutralBackground2); min-width:0; }
[data-sc-settings-fluent] main > div { padding:24px; }
[data-sc-settings-fluent] main > div > div { max-width:1000px; margin-inline:auto; width:100%; }
.sc-settings-search-bar {
  display:flex; align-items:center; gap:16px; height:64px;
  padding:0 20px 0 24px; background:var(--colorNeutralBackground2);
  border-bottom:1px solid var(--colorNeutralStroke2);
}
.sc-settings-search-bar h2 { font-size:20px; font-weight:600; margin:0; flex:0 0 220px; }
.sc-settings-search-bar > div { flex:1; max-width:720px; padding:0; }
.sc-settings-search-bar input {
  font:inherit; font-size:14px; min-height:32px;
  color:var(--colorNeutralForeground1);
}
.sc-settings-search-bar > div > div > div:first-child {
  background:var(--colorNeutralBackground1); border:1px solid var(--colorNeutralStroke1);
  border-bottom:1px solid var(--colorNeutralStrokeAccessible); border-radius:4px;
}
.sc-settings-search-bar > div:focus-within > div > div:first-child { border-bottom:2px solid var(--sc-accent-indicator); }
.sc-settings-search-bar > button { margin-left:auto; width:32px; height:32px; padding:6px; border-radius:4px; }
[data-sc-settings-fluent] [data-nav-id] {
  position:relative; border-radius:5px; padding:10px 14px; min-height:40px;
  font:14px var(--fontFamilyBase); color:var(--colorNeutralForeground1); gap:14px;
}
[data-sc-settings-fluent] [data-nav-id]:hover { background:var(--colorSubtleBackgroundHover); }
[data-sc-settings-fluent] [data-nav-id][class*="surface-selected"] { background:var(--colorSubtleBackgroundSelected); }
[data-sc-settings-fluent] [data-nav-id][class*="surface-selected"]::before {
  content:""; position:absolute; left:0; top:calc(50% - 8px); width:3px;
  height:16px; border-radius:2px; background:var(--sc-accent-indicator);
}
[data-sc-settings-fluent] nav nav { padding:4px 12px 16px; gap:4px; }
[data-sc-settings-fluent] nav h3 { font-size:12px; font-weight:600; text-transform:none; color:var(--colorNeutralForeground2); }
[data-sc-settings-fluent] .setting-group { margin:0 0 24px; }
[data-sc-settings-fluent] .setting-group > h3 { font-size:18px; font-weight:600; margin:0 0 12px; }
[data-sc-settings-fluent] .setting-group > div:first-child:not([data-setting-id]) { display:none; }
[data-sc-settings-fluent] [data-sc-setting-kind] {
  min-height:68px; padding:16px; margin:0 0 4px; border-radius:6px;
  background:var(--colorNeutralBackground1); border:1px solid var(--colorNeutralStroke2);
}
[data-sc-settings-fluent] [data-sc-setting-kind] .form-label { flex-direction:column; align-items:flex-start; justify-content:center; gap:4px; min-width:0; }
[data-sc-settings-fluent] [data-sc-setting-kind] .form-label > span { color:var(--colorNeutralForeground1); font:14px var(--fontFamilyBase); }
[data-sc-settings-fluent] [data-sc-setting-kind] > div { gap:24px; }
[data-sc-settings-fluent] [data-sc-setting-kind="standard"] .form-input { flex-shrink:0; max-width:48%; }
.sc-setting-description { color:var(--colorNeutralForeground2); font-size:12px; line-height:1.5; white-space:pre-wrap; }
[data-sc-settings-popup] {
  background:var(--colorNeutralBackground1); color:var(--colorNeutralForeground1);
  border:1px solid var(--colorNeutralStroke1); border-radius:6px;
  box-shadow:var(--shadow16); font:14px var(--fontFamilyBase);
}
[data-sc-settings-popup] .p-select-list { padding:4px; }
[data-sc-settings-popup] .p-select-option { padding:8px 12px; border-radius:4px; min-height:32px; color:inherit; }
[data-sc-settings-popup] .p-select-option:not([aria-selected="true"]):hover { background:var(--colorSubtleBackgroundHover); }
[data-sc-settings-popup] .p-select-option[aria-selected="true"] { background:var(--colorSubtleBackgroundSelected); color:var(--sc-accent-indicator); }
[data-sc-settings-fluent] [data-sc-setting-kind] :is(.p-inputtext,.p-select,select,input[type="text"],input[type="number"]) {
  background:var(--colorNeutralBackground1); color:var(--colorNeutralForeground1);
  border:1px solid var(--colorNeutralStroke1); border-bottom-color:var(--colorNeutralStrokeAccessible);
  border-radius:4px; font:14px var(--fontFamilyBase); min-height:32px;
}
[data-sc-settings-fluent] [data-sc-setting-kind] :is(select,input[type="text"],input[type="number"]) { padding:5px 10px; }
[data-sc-settings-fluent] [data-sc-setting-kind] .p-select-label { padding:6px 12px; }
[data-sc-settings-fluent] [data-sc-setting-kind] :is(input,select,button):focus-visible { outline:2px solid var(--sc-accent-indicator); outline-offset:2px; }
[data-sc-settings-fluent] [data-sc-setting-kind] .p-toggleswitch { width:40px; height:20px; }
[data-sc-settings-fluent] [data-sc-setting-kind] .p-toggleswitch-slider { border:1px solid var(--colorNeutralStrokeAccessible); border-radius:10px; background:transparent; }
[data-sc-settings-fluent] [data-sc-setting-kind] .p-toggleswitch-handle { width:12px; height:12px; margin-top:-6px; left:4px; background:var(--colorNeutralForeground2); }
[data-sc-settings-fluent] [data-sc-setting-kind] .p-toggleswitch-checked .p-toggleswitch-slider { background:var(--sc-accent-indicator); border-color:transparent; }
[data-sc-settings-fluent] [data-sc-setting-kind] .p-toggleswitch-checked .p-toggleswitch-handle { left:24px; background:var(--sc-accent-foreground); }
[data-sc-settings-fluent] [data-sc-setting-kind] button {
  background:var(--colorNeutralBackground1); color:var(--colorNeutralForeground1);
  border:1px solid var(--colorNeutralStroke1); border-radius:4px; min-height:32px;
  font:14px var(--fontFamilyBase); padding:5px 12px;
}
[data-sc-settings-fluent] [data-sc-setting-kind] button:hover { background:var(--colorNeutralBackground1Hover); }
[data-sc-settings-fluent] [data-sc-setting-kind] :disabled { opacity:.5; cursor:default; }
[data-sc-settings-fluent] [data-sc-setting-kind] input[type="checkbox"] { accent-color:var(--sc-accent-indicator); }
[data-sc-settings-fluent] .sc-settings-about__card:has(.sc-settings-about__actions) { display:block; }
[data-sc-settings] .sc-settings-about__card + .sc-settings-about__card { margin-top:4px; }
[data-sc-settings] .sc-settings-about__versions > .sc-settings-about__card { margin:0; }
[data-sc-settings-fluent] [data-setting-id="SugarCubes.About.Information"] { background:transparent; padding:0; border:0; }
@media(max-width:800px) {
  [data-sc-settings-fluent] > div { grid-template-columns:180px minmax(0,1fr) !important; }
  .sc-settings-search-bar h2 { flex-basis:140px; }
  [data-sc-settings-fluent] main > div { padding:16px; }
  [data-sc-settings-fluent] [data-sc-setting-kind] > div { flex-wrap:wrap; gap:12px; }
  [data-sc-settings-fluent] [data-sc-setting-kind="standard"] .form-input { max-width:100%; }
}
@media(prefers-reduced-motion:no-preference) {
  [data-sc-settings-fluent] main > div > div { animation:sc-settings-enter 160ms cubic-bezier(.1,.9,.2,1); }
  @keyframes sc-settings-enter { from { opacity:0; transform:translateY(8px); } to { opacity:1; transform:none; } }
}
`;
