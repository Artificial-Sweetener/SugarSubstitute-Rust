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
/** Apply Substitute's compact banner and justified-wall presentation to picker-owned surfaces. */
const findIcon = new URL('../../../icons/model/download.svg', import.meta.url).href;
const showAllIcon = new URL('../../../icons/model/eye.svg', import.meta.url).href;
const showOnlyIcon = new URL('../../../icons/model/eye-off.svg', import.meta.url).href;
import { modelRecommendationPreviewCss } from './ModelRecommendationPreviewStyles.js';
import { modelKeyRequiredBadgeCss } from './vue/ModelKeyRequiredBadge.js';
/** Style only picker-owned fields, navigation, menus, and reference galleries. */
export const modelPickerCss =
  modelRecommendationPreviewCss +
  modelKeyRequiredBadgeCss +
  `
.sc-model-find { display:inline-flex; align-items:center; justify-content:center; gap:8px; width:100%; height:32px; padding:0 12px; border:1px solid transparent; border-radius:4px; background:var(--colorBrandBackground,#0078d4); color:var(--colorNeutralForegroundOnBrand,#fff); font:12px 'Segoe UI Variable','Segoe UI',sans-serif; cursor:pointer; }
.sc-model-find:hover { background:var(--colorBrandBackgroundHover,#106ebe); }
.sc-model-find-icon { width:16px; height:16px; background:currentColor; mask:url("${findIcon}") center/contain no-repeat; }
.sc-model-field { position:relative; display:flex; align-items:center; box-sizing:border-box; width:100%; max-width:100%; min-width:0; height:32px; overflow:hidden; border:1px solid var(--colorNeutralStroke1,#595959); border-bottom-color:var(--colorNeutralStrokeAccessible,#929292); border-radius:5px; background:var(--colorNeutralBackground1,#383838); color:var(--colorNeutralForeground1,#eee); font:12px 'Segoe UI Variable','Segoe UI',sans-serif; }
.sc-model-field::after { content:''; position:absolute; inset:auto 0 0; height:2px; background:var(--colorBrandStroke1,#4ca6f9); transform:scaleX(0); transform-origin:center; transition:transform var(--durationNormal,200ms) var(--curveDecelerateMid,cubic-bezier(.1,.9,.2,1)); pointer-events:none; }
.sc-model-field-open::after,.sc-model-field:focus-within::after { transform:scaleX(1); }
.sc-model-field input { position:relative; flex:1; width:0; min-width:0; height:100%; padding:0 11px; color:inherit; background:transparent; border:0; outline:0; font:inherit; text-overflow:ellipsis; cursor:pointer; }
.sc-model-field-open input { cursor:text; }
.sc-model-field-transferring { overflow:hidden; }
.sc-model-field-transferring input { color:transparent; }
.sc-model-transfer-presentation { position:absolute; inset:0; pointer-events:none; }
.sc-model-transfer-thumbnail { position:absolute; inset:0; width:100%; height:100%; object-fit:cover; filter:brightness(.55); }
.sc-model-transfer-caption { position:absolute; z-index:1; inset:0 30px 0 11px; display:flex; flex-direction:column; justify-content:center; font-size:0.9em; line-height:1.05; }
.sc-model-transfer-caption > span { white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
.sc-model-transfer-fill { position:absolute; inset:0 auto 0 0; height:100%; background:var(--colorBrandBackground,var(--primary-background,#4ca6f9)); opacity:.55; pointer-events:none; }
.sc-model-transfer-indeterminate { width:35%; animation:sc-model-transfer 1.2s ease-in-out infinite; }
@keyframes sc-model-transfer { from { left:-35%; } to { left:100%; } }
@media(prefers-reduced-motion:reduce) { .sc-model-transfer-indeterminate { animation:none; width:100%; opacity:.15; } }
.sc-model-completion { position:absolute; left:11px; right:30px; overflow:hidden; white-space:pre; color:var(--colorNeutralForeground3,#999); pointer-events:none; }
.sc-model-completion-prefix { color:transparent; }
.sc-model-banner { position:absolute; inset:0; width:100%; height:100%; object-fit:cover; filter:brightness(.55); pointer-events:none; }
.sc-model-field-arrow { position:relative; display:grid; place-items:center; flex:none; width:30px; height:100%; color:inherit; border:0; background:transparent; cursor:pointer; }
.sc-model-field:has(input:disabled) { opacity:.6; }
.sc-model-field-transferring:has(input:disabled) { opacity:1; }
[data-sc-model-appearance="native"] .sc-model-field { height:24px; border:0; border-radius:6px; background:var(--component-node-widget-background); color:var(--component-node-foreground); font:inherit; font-size:12px; line-height:16px; }
[data-sc-model-appearance="native"] .sc-model-field:hover { background:var(--component-node-widget-background-hovered); }
[data-sc-model-appearance="native"] .sc-model-field:has(input:disabled) { background:var(--component-node-widget-background-disabled); }
[data-sc-model-appearance="native"] .sc-model-field::after { display:none; }
[data-sc-model-appearance="native"] .sc-model-field:focus-within { outline:1px solid var(--primary-background); outline-offset:-1px; }
[data-sc-model-appearance="native"] .sc-model-field input { padding:0 8px; }
[data-sc-model-appearance="native"] .sc-model-field-arrow { width:24px; color:var(--muted-foreground); }
[data-sc-model-appearance="native"] .sc-model-completion { left:8px; right:24px; color:var(--component-node-foreground-secondary); }
[data-sc-model-appearance="native"] .sc-model-banner { display:none; }
[data-sc-model-appearance="native"] .sc-model-transfer-caption { left:8px; right:24px; }
.sc-model-popup[data-sc-model-appearance="native"] { background:var(--p-popover-background); color:var(--p-popover-color); border-color:var(--p-popover-border-color); border-radius:var(--p-popover-border-radius); box-shadow:var(--p-popover-shadow); font:12px/16px var(--font-sans,Inter,Arial,sans-serif); --colorBrandBackground:var(--primary-background); --colorNeutralForeground2:var(--component-node-foreground-secondary); --colorNeutralBackground1Hover:var(--component-node-widget-background-hovered); }
.sc-model-chevron { width:7px; height:7px; border-right:1.5px solid currentColor; border-bottom:1.5px solid currentColor; transform:translateY(-2px) rotate(45deg); }
.sc-model-popup { position:fixed; z-index:12000; display:flex; flex-direction:column; box-sizing:border-box; padding:10px; overflow:hidden; border:1px solid var(--colorNeutralStroke1,#404040); border-radius:5px; background:var(--colorNeutralBackground1,#282828); color:var(--colorNeutralForeground1,#eee); box-shadow:0 8px 24px #0006; font:12px 'Segoe UI Variable','Segoe UI',sans-serif; }
.sc-model-routes { display:flex; flex-direction:column; gap:4px; flex:none; min-width:0; padding-bottom:4px; }
.sc-model-breadcrumbs,.sc-model-folder-strip { display:flex; align-items:center; gap:4px; min-width:0; min-height:24px; box-sizing:border-box; padding-bottom:0; overflow-x:auto; overscroll-behavior:contain; }
/* Keep floating horizontal scrollbar hit targets below buttons in overflowing rows. */
.sc-model-route-overflow { min-height:36px; padding-bottom:12px; }
.sc-model-popup-above .sc-model-routes { order:1; padding-top:4px; padding-bottom:0; }
.sc-model-routes button { box-sizing:border-box; min-height:0; height:24px; margin:0; padding:0 4px; min-width:0; flex:none; white-space:nowrap; border:0; border-radius:3px; color:inherit; background:transparent; font:inherit; line-height:19px; cursor:pointer; }
.sc-model-routes button:hover { background:var(--colorNeutralBackground1Hover,#ffffff12); }
.sc-model-routes .sc-model-filter-toggle { display:inline-flex; align-items:center; gap:4px; margin-left:auto; padding:2px 6px; }
.sc-model-filter-toggle[aria-pressed="true"] { background:var(--colorNeutralBackground1Hover,#ffffff12); }
.sc-model-filter-icon { display:block; width:16px; height:16px; flex:none; background:currentColor; mask:url("${showAllIcon}") center/contain no-repeat; }
.sc-model-filter-toggle[aria-pressed="true"] .sc-model-filter-icon { mask:url("${showOnlyIcon}") center/contain no-repeat; }
.sc-model-route-separator { flex:none; color:var(--colorNeutralForeground3,#aaa); }
.sc-model-child-folder { color:var(--colorNeutralForeground2,#c5c5c5)!important; }
.sc-model-wall-viewport { flex:1; min-height:0; overflow-y:auto; overflow-x:hidden; overscroll-behavior:contain; }
.sc-model-wall { position:relative; width:100%; }
.sc-model-tile { position:absolute; display:block; box-sizing:border-box; overflow:hidden; padding:0; border:0; background:#ffffff0d; color:#fff; text-align:left; cursor:pointer; }
.sc-model-tile img { display:block; width:100%; height:100%; object-fit:cover; pointer-events:none; }
.sc-model-tile::after { content:''; position:absolute; inset:0; pointer-events:none; background:linear-gradient(transparent,rgba(0,0,0,.05) 42%,rgba(0,0,0,.70)); }
.sc-model-tile:hover::after,.sc-model-active::after,.sc-model-current::after { background:linear-gradient(transparent,rgba(0,0,0,.09) 42%,rgba(0,0,0,.86)); }
.sc-model-tile:hover { outline:1px solid #ffffff5a; outline-offset:-1px; }
.sc-model-current,.sc-model-active { outline:2px solid var(--colorBrandBackground,#4ca6f9)!important; outline-offset:-2px; }
.sc-model-caption { position:absolute; z-index:1; inset:auto 8px 7px; display:flex; flex-direction:column; gap:2px; line-height:17px; pointer-events:none; }
.sc-model-title,.sc-model-version { display:block; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
.sc-model-title { color:#ffffffeb; }
.sc-model-title[data-marquee-phase="scroll"] { text-overflow:clip; mask-image:linear-gradient(to right,transparent,#000 18px,#000 calc(100% - 18px),transparent); }
.sc-model-title[data-marquee-phase="scroll"][data-marquee-left-fade="false"] { mask-image:linear-gradient(to right,#000,#000 calc(100% - 18px),transparent); }
.sc-model-title[data-marquee-phase="scroll"][data-marquee-right-fade="false"] { mask-image:linear-gradient(to right,transparent,#000 18px,#000); }
.sc-model-title[data-marquee-phase="end"] { direction:rtl; text-align:left; }
.sc-model-marquee-text { unicode-bidi:embed; }
.sc-model-title[data-marquee-phase="scroll"] .sc-model-marquee-text { display:inline-block; }
.sc-model-version { color:#ffffffaf; }
.sc-model-no-image { position:absolute; inset:0; display:grid; place-items:center; color:#ffffff96; }
.sc-model-empty { display:grid; flex:1; place-items:center; color:var(--colorNeutralForeground2,#bbb); }
.sc-model-provider-icon { display:block; flex:none; width:16px; height:16px; object-fit:contain; }
span.sc-model-provider-icon { background:currentColor; mask:var(--sc-model-provider-icon) center/contain no-repeat; }
.sc-model-update { position:relative; z-index:2; display:grid; place-items:center; flex:none; width:26px; height:25px; padding:0; background:transparent; border:0; border-radius:5px; color:inherit; cursor:pointer; }
.sc-model-update:hover,.sc-model-update:focus-visible { background:#ffffff30; outline:0; }
.sc-model-update span { display:block; width:18px; height:18px; background:currentColor; mask:var(--sc-model-update-icon) center/contain no-repeat; }
.sc-model-tile .sc-model-update { position:absolute; top:5px; right:5px; background:#0008; }
.sc-model-version-overlay { z-index:14000; }
.sc-model-detector-overlay { z-index:14000; }
.sugarcubes-modal.sc-model-detector-modal { display:flex; flex-direction:column; width:min(880px,calc(100vw - 32px)); max-width:880px; height:min(650px,calc(100vh - 32px)); max-height:calc(100vh - 32px); overflow:hidden; }
.sc-model-detector-modal .p-dialog-header { flex:none; }
.sc-model-detector-modal .p-dialog-footer { flex:none; box-sizing:border-box; height:81px; }
.sc-model-detector-modal .sugarcubes-modal__footer-meta { display:none; }
.sc-model-detector-modal .sugarcubes-modal__footer-actions { display:flex; justify-content:flex-end; width:100%; }
.sc-model-detector-modal .sugarcubes-modal__footer-actions button:not([hidden]) { width:50%; }
.sc-model-detector-modal .p-dialog-content { flex:1; min-height:0; display:flex; padding:0 24px 24px; }
.sc-model-detector-body,.sc-model-detector-wall { display:flex; flex-direction:column; flex:1; min-height:0; min-width:0; outline:0; }
.sugarcubes-modal.sc-model-version-modal { display:flex; flex-direction:column; width:fit-content; min-width:min(480px,calc(100vw - 32px)); max-width:min(1040px,calc(100vw - 32px)); max-height:calc(100vh - 32px); overflow:hidden; }
.sugarcubes-modal.sc-model-version-modal .p-dialog-content { padding:0 24px; }
.sugarcubes-modal.sc-model-version-modal .p-dialog-footer { padding:8px 24px 20px; }
.sc-model-key-overlay { z-index:15000; }
.sc-model-key-dialog .sc-model-key-explanation { margin:0 0 16px; line-height:1.5; }
.sc-model-key-dialog .sugarcubes-modal__field { display:flex; flex-direction:column; gap:8px; }
.sc-model-key-dialog .sc-model-key-settings { display:inline-block; margin-top:12px; }
.sc-model-version-loading { display:flex; align-items:center; justify-content:center; gap:8px; width:432px; max-width:100%; height:256px; }
.sc-model-timeline { display:flex; gap:12px; width:max-content; max-width:100%; margin-inline:auto; height:auto; padding-bottom:4px; overflow-x:auto; overflow-y:hidden; overscroll-behavior:contain; }
.sc-model-version-card { position:relative; display:flex; flex-direction:column; align-items:center; gap:6px; box-sizing:border-box; flex:none; width:204px; height:256px; margin:0; padding:9px 9px 7px; border:1px solid #ffffff1c; border-radius:13px; background:#ffffff07; color:inherit; font:inherit; cursor:pointer; }
.sc-model-version-selected { border-color:var(--colorBrandBackground,#0078d4); box-shadow:inset 0 0 0 1px var(--colorBrandBackground,#0078d4); }
.sc-model-version-card:disabled { cursor:default; color:inherit; }
.sc-model-version-portrait { position:relative; display:block; flex:none; box-sizing:border-box; width:100%; height:200px; overflow:hidden; border-radius:14px; background:#ffffff0c; }
.sc-model-version-portrait .sc-model-key-required { left:12px; top:12px; z-index:2; }
.sc-model-version-name { position:absolute; inset:42% 0 0; display:flex; align-items:flex-end; box-sizing:border-box; padding:16px 16px 15px; background:linear-gradient(transparent,rgba(6,9,14,.13) 56%,rgba(6,9,14,.52) 78%,rgba(6,9,14,.91)); overflow:hidden; color:#f8f9fc; font-size:17.333px; font-weight:700; line-height:1.2; text-align:left; overflow-wrap:anywhere; }
.sc-model-version-check { position:absolute; top:12px; right:12px; display:grid; place-items:center; box-sizing:border-box; width:20px; height:20px; border:1px solid #fff9; border-radius:4px; color:#fff; background:#0008; }
.sc-model-version-check svg { display:block; width:14px; height:14px; }
.sc-model-version-selected .sc-model-version-check { background:var(--colorBrandBackground,#0078d4); border-color:var(--colorBrandBackground,#0078d4); color:var(--colorNeutralForegroundOnBrand,#fff); }
.sc-model-version-state { display:flex; align-items:center; justify-content:center; align-self:stretch; flex:1; color:var(--colorNeutralForeground2,#ccc); background:var(--colorNeutralBackground2,#202024); }
.sc-model-family-footer-meta { display:flex; align-items:center; gap:12px; }
.sc-model-family-footer-meta a { display:inline-flex; align-items:center; gap:8px; box-sizing:border-box; height:32px; color:inherit; font-size:14px; text-decoration:none; padding:0 12px; border:1px solid var(--colorNeutralStroke1,#666); border-radius:4px; background:var(--colorNeutralBackground1,#383838); }
.sc-model-action-icon { display:inline-block; flex:none; width:16px; height:16px; background:currentColor; mask:var(--sc-model-action-icon) center/contain no-repeat; }
.sc-model-popup { animation:sc-model-popup-in var(--durationFast,150ms) var(--curveDecelerateMid,cubic-bezier(.1,.9,.2,1)); }
@keyframes sc-model-popup-in { from { opacity:0; transform:translateY(4px); } to { opacity:1; transform:translateY(0); } }
@media(prefers-reduced-motion:reduce) { .sc-model-field::after { transition:none; } .sc-model-popup { animation:none; } }
`;
