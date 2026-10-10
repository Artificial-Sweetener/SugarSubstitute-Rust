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
/** Share reference loading and unavailable presentation without changing portrait geometry. */
import { modelBusyRingCss } from './ModelBusyRingStyles.js';

/** Retain contrast for white image captions while a thumbnail is absent or loading. */
export const MODEL_PREVIEW_BACKDROP = '#202024';

/** Position status above captions using the reference portrait's reserved geometry. */
export const modelRecommendationPreviewCss =
  modelBusyRingCss +
  `
.sc-model-recommendation-preview { position:absolute; inset:0; display:block; pointer-events:none; }
.sc-model-recommendation-preview img { position:absolute; inset:0; width:100%; height:100%; object-fit:cover; }
.sc-model-preview-unavailable { position:absolute; inset:0 16px 34px; z-index:2; display:grid; place-items:center; color:#f8f9fcd2; font:12px 'Segoe UI',sans-serif; }
.sc-model-preview-loading { position:absolute; inset:0; z-index:2; color:#f8f9fcd2; font:12px 'Segoe UI',sans-serif; }
.sc-model-preview-loading-label { position:absolute; top:calc(32% + 20px); left:16px; right:16px; height:28px; display:grid; place-items:center; }
.sc-model-preview-busy-ring { position:absolute; top:calc(32% - 17px); left:calc(50% - 17px); }
`;
