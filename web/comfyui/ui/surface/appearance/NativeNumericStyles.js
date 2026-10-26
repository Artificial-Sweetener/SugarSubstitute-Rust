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
/** Arrange native number spinners and Substitute's slider pairs through themed styling. */
import { CONTROL_HEIGHT, SPINNER_WIDTH } from './ControlGeometry.js';
/** Keep number geometry scoped to positively identified native components in both views. */
export function nativeNumericCss() {
    return `
    [data-sc-number-spinner] {
      position:relative; display:flex; box-sizing:border-box; padding-right:var(--sc-spinner-stepper-width,20px);
      height:${CONTROL_HEIGHT}; border:var(--sc-field-border-width) solid var(--sc-control-stroke); border-bottom-color:var(--sc-control-bottom-stroke);
      border-radius:var(--sc-field-radius); background:var(--sc-control-fill); overflow:hidden;
    }
    [data-sc-number-spinner] > div:has(> [data-sc-number-input]) { margin:0; padding:0; height:100%; min-width:0; }
    [data-sc-number-spinner]:not([data-sc-seed-box]) { width:${SPINNER_WIDTH}; max-width:100%; flex:none; font-variant-numeric:tabular-nums; }
    [data-sc-number-input] { box-sizing:border-box; min-width:0; width:100%; border:0; margin:0; height:100%; text-align:center; padding:0 8px; }
    [data-sc-native-number-fill] { display:none; }
    [data-sc-number-increment], [data-sc-number-decrement] {
      position:absolute; right:0; width:20px; height:50%; min-height:0; padding:0; margin:0;
      border:0; border-left:1px solid var(--sc-control-stroke); border-radius:0;
      background:transparent; display:flex; align-items:center; justify-content:center;
    }
    [data-sc-number-increment] { top:0; }
    [data-sc-number-decrement] { bottom:0; }
    [data-sc-number-increment] i, [data-sc-number-decrement] i,
    [data-sc-number-increment] span, [data-sc-number-decrement] span { font-size:10px; width:10px; height:10px; }
    [data-sc-number-increment]:hover, [data-sc-number-decrement]:hover { background:var(--sc-surface-hover); }
    [data-sc-number-increment]:active, [data-sc-number-decrement]:active { background:var(--sc-surface-pressed); }
    [data-sc-number-slider] { cursor:default !important; }
    [data-sc-number-slider]::part(track-container), [data-sc-number-slider] [data-sc-slider-thumb], [data-sc-number-slider] [data-sc-slider-thumb] * { cursor:default !important; }
    [data-sc-number-slider]::after { content:none; }
    [data-sc-number-slider]::part(track-container) { border-radius:var(--border-radius); }
    [data-sc-number-slider]::part(track-container)::after { background-image:none; }
  `;
}
