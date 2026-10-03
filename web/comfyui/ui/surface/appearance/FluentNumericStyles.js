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
/** Arrange native number spinners and Substitute's slider pairs through Fluent styling. */
/** Keep number geometry scoped to positively identified native components in both views. */
export function fluentNumericCss() {
    return `
    [data-sc-number-spinner] {
      position:relative; display:flex; box-sizing:border-box; padding-right:20px;
      height:33px; border:1px solid var(--sc-control-stroke); border-bottom-color:var(--sc-control-bottom-stroke);
      border-radius:4px; background:var(--sc-control-fill); overflow:hidden;
    }
    [data-sc-number-spinner] > div:has(> [data-sc-number-input]) { margin:0; padding:0; height:100%; min-width:0; }
    [data-sc-number-input] { min-width:0; width:100%; border:0; margin:0; height:100%; text-align:center; padding-inline:0; }
    [data-sc-number-increment], [data-sc-number-decrement] {
      position:absolute; right:0; width:20px; height:50%; min-height:0; padding:0; margin:0;
      border:0; border-left:1px solid var(--sc-control-stroke); border-radius:0;
      background:transparent; display:flex; align-items:center; justify-content:center;
    }
    [data-sc-number-increment] { top:0; }
    [data-sc-number-decrement] { bottom:0; }
    [data-sc-number-increment] i, [data-sc-number-decrement] i,
    [data-sc-number-increment] span, [data-sc-number-decrement] span { font-size:10px; width:10px; height:10px; }
    [data-sc-number-increment]:hover, [data-sc-number-decrement]:hover { background:var(--colorNeutralBackground1Hover); }
    [data-sc-number-increment]:active, [data-sc-number-decrement]:active { background:var(--colorNeutralBackground1Pressed); }
    [data-sc-number-pair] { display:flex; align-items:center; justify-content:flex-end; min-width:0; gap:6px; }
    [data-sc-number-pair] > :first-child { width:80px; min-width:64px; flex:0 1 80px; }
    [data-sc-number-slider] { width:120px; min-width:40px; max-width:120px; flex:1 1 120px; cursor:default; }
    [data-sc-number-slider]::part(track-container)::after { background-image:none; }
    .sugarcubes-workflow-masonry [data-sc-fluent-row][data-sc-fluent-widget^="WidgetInputNumber"] [data-sc-fluent-layout] > [data-sc-number-pair] {
      flex:0 0 206px; max-width:206px;
    }
  `;
}
