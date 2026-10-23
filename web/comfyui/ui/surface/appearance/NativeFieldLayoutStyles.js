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
/** Keep labels and values at opposite field edges across native presentation surfaces. */
import { CONTROL_HEIGHT, SPINNER_WIDTH } from './ControlGeometry.js';
/** Reuse native scalar, model and numeric field sizing within one identified surface. */
export function nativeFieldLayoutCss(scope) {
    return `
    ${scope} [data-sc-style-layout] {
      display:flex; flex-direction:row; align-items:center; justify-content:space-between; gap:6px;
    }
    ${scope} [data-sc-style-layout] > :first-child { min-width:0; flex:1; }
    ${scope} [data-sc-style-layout]:not([data-sc-choice-inline] *) > :last-child { min-width:0; flex:0 1 156px; max-width:65%; }
    ${scope} [data-sc-style-row][data-sc-style-widget^="WidgetInputNumber"] [data-sc-style-layout] > :last-child {
      flex-basis:${SPINNER_WIDTH};
    }
    ${scope} [data-sc-style-row][data-sc-style-widget="WidgetToggleSwitch"] [data-sc-style-layout] > :last-child {
      flex:0 0 auto;
    }
    ${scope} [data-sc-style-row] [data-sc-style-layout]:not([data-sc-choice-inline] *) > :last-child:has(.sc-model-mount) {
      flex-basis:360px; max-width:min(65%,360px);
    }
    ${scope} [data-sc-style-row][data-sc-style-widget^="WidgetInputNumber"] [data-sc-style-layout] > :last-child > :first-child > :first-child {
      height:${CONTROL_HEIGHT}; border:var(--sc-field-border-width) solid var(--sc-control-stroke);
      border-bottom-color:var(--sc-control-bottom-stroke); border-radius:var(--sc-field-radius);
      background:var(--sc-control-fill);
    }
  `;
}
