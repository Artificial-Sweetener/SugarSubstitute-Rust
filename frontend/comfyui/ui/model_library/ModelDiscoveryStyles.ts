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
/** Match Substitute's portrait discovery surface and contained Windows-style credential layer. */

import {
  modelRecommendationPreviewCss,
  MODEL_PREVIEW_BACKDROP,
} from './ModelRecommendationPreviewStyles.js';
import { modelKeyRequiredBadgeCss } from './vue/ModelKeyRequiredBadge.js';
const eyeIcon = new URL('../../../icons/model/eye.svg', import.meta.url).href;

/** Share reference geometry across owned discovery and credential surfaces. */
export const modelDiscoveryCss =
  modelRecommendationPreviewCss +
  modelKeyRequiredBadgeCss +
  `
.sugarcubes-modal.sc-model-discovery-modal { position:relative; display:flex; flex-direction:column; width:min(1040px,calc(100vw - 32px)); height:min(720px,calc(100vh - 32px)); max-width:1040px; max-height:720px; overflow:hidden; }
.sc-model-discovery-modal .p-dialog-content { position:static; display:flex; flex:1; min-height:0; padding:0 24px 12px; overflow:hidden; }
.sc-model-discovery-modal .sugarcubes-modal__description,.sc-model-discovery-modal .sugarcubes-modal__error { display:none; }
.sc-model-discovery-body,.sc-model-discovery-view { display:flex; flex-direction:column; flex:1; min-height:0; min-width:0; }
.sc-model-discovery-purpose { flex:none; margin:0; padding:0; font-size:12px; }
.sc-model-discovery-grid { display:grid; flex:1; min-height:0; overflow:auto; margin-top:12px; grid-template-columns:repeat(4,204px); grid-template-rows:repeat(2,220px); align-content:space-evenly; justify-content:space-evenly; gap:10px; }
.sc-model-discovery-card { position:relative; width:204px; height:220px; }
.sc-model-discovery-portrait { position:absolute; inset:10px; display:block; width:184px; height:200px; padding:0; border:0; border-radius:14px; overflow:hidden; background:${MODEL_PREVIEW_BACKDROP}; color:#fff; cursor:pointer; text-align:left; }
.sc-model-discovery-portrait::before { content:''; position:absolute; inset:0; z-index:1; background:linear-gradient(to bottom,rgba(6,9,14,0) 42%,rgba(6,9,14,.133) 74.48%,rgba(6,9,14,.518) 87.24%,rgba(6,9,14,.91)); pointer-events:none; }
.sc-model-discovery-portrait:hover::before { background:linear-gradient(to bottom,rgba(6,9,14,0) 42%,rgba(6,9,14,.227) 74.48%,rgba(6,9,14,.651) 87.24%,rgba(6,9,14,.91)); }
.sc-model-discovery-portrait:focus-visible { outline:2px solid var(--colorBrandStroke1,#4ca6f9); outline-offset:2px; }
.sc-model-discovery-caption { position:absolute; inset:auto 0 0; z-index:2; padding:16px 16px 15px; font:700 17.333px/1.35 'Segoe UI',sans-serif; overflow-wrap:anywhere; pointer-events:none; }
.sc-model-discovery-check { position:absolute; right:20px; top:16px; z-index:2; display:grid; place-items:center; width:20px; height:20px; border:1px solid #ffffff70; border-radius:4px; font:14px 'Segoe UI',sans-serif; background:#ffffff08; }
.sc-model-discovery-portrait.is-selected .sc-model-discovery-check { background:var(--colorBrandBackground,#0078d4); border-color:transparent; }
.sc-model-discovery-provider { position:absolute; left:20px; top:20px; width:28px; height:28px; display:grid; place-items:center; padding:0; border:0; border-radius:50%; background:transparent; cursor:pointer; }
.sc-model-discovery-provider img { width:20px; height:20px; object-fit:contain; }
.sc-model-discovery-card .sc-model-key-required { left:50px; top:22px; }
.sc-model-discovery-loading { display:flex; flex:1; align-items:center; justify-content:center; gap:12px; font-size:14px; }
.sc-model-discovery-status { display:flex; align-items:center; gap:12px; flex:none; padding:8px 0; font-size:14px; }
.sc-model-discovery-modal .p-dialog-footer { flex:none; }
.sc-model-discovery-footer { display:flex; align-items:center; gap:8px; }
.sc-model-credential-layer { position:absolute; inset:0; z-index:5; display:flex; align-items:center; justify-content:center; background:rgba(0,0,0,.706); }
.sc-model-credential-panel { box-sizing:border-box; width:min(560px,calc(100% - 24px)); display:flex; flex-direction:column; gap:14px; padding:22px 24px; }
.sc-model-credential-panel h2 { margin:0; }
.sc-model-credential-panel p { margin:0; padding:0 0 4px; white-space:pre-line; line-height:1.4; }
.sc-model-credential-panel>a { align-self:flex-start; }
.sc-model-credential-input { display:flex; height:33px; overflow:hidden; }
.sc-model-credential-input input { flex:1; min-width:0; padding:0 10px; color:inherit; background:transparent; border:0; outline:0; font:inherit; }
.sc-model-credential-input button { flex:none; display:grid; place-items:center; width:32px; padding:0; border:0; background:transparent; color:inherit; cursor:pointer; }
.sc-model-credential-eye { width:16px; height:16px; background:currentColor; mask:url("${eyeIcon}") center/contain no-repeat; }
.sc-model-credential-actions { display:flex; justify-content:flex-end; gap:8px; padding-top:10px; }
.sc-model-credential-error { color:var(--colorPaletteRedForeground1,#f98989); }
@media(max-width:930px) { .sc-model-discovery-grid { grid-template-columns:repeat(3,204px); grid-template-rows:none; grid-auto-rows:220px; align-content:start; } }
@media(max-width:710px) { .sc-model-discovery-grid { grid-template-columns:repeat(2,204px); } .sc-model-credential-actions { flex-wrap:wrap; } }
@media(max-width:460px) { .sc-model-discovery-grid { grid-template-columns:204px; } }
`;
