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
/** Paint a concentric animated dot through the official Fluent slider thumb slot. */
/** Supply a decorative thumb while Fluent retains focus, pointer gestures, and value ownership. */
export function createFluentSliderThumb(documentRef) {
    const thumb = documentRef.createElement('span');
    thumb.slot = 'thumb';
    thumb.dataset.scSliderThumb = '';
    thumb.setAttribute('aria-hidden', 'true');
    return thumb;
}
/** Paint ring and dot on one surface so graph zoom cannot snap their centers independently. */
export function fluentSliderThumbCss() {
    return `
    [data-sc-slider-thumb] {
      display:block; box-sizing:border-box;
      width:var(--thumb-size); height:var(--thumb-size); border-radius:50%;
      border:1px solid var(--colorNeutralStroke1); background-color:var(--colorNeutralBackground1);
      background-image:radial-gradient(circle closest-side,var(--rail-color) calc(100% - .5px),transparent 100%);
      background-position:center; background-repeat:no-repeat; background-size:10px 10px;
      transition:background-size var(--durationFaster,100ms) var(--curveEasyEase);
    }
    [data-sc-number-slider]:not([disabled]) [data-sc-slider-thumb]:hover { background-size:12px 12px; }
    [data-sc-number-slider]:not([disabled]):active [data-sc-slider-thumb] { background-size:8px 8px; }
    @media (prefers-reduced-motion:reduce) { [data-sc-slider-thumb] { transition:none; } }
    @media (forced-colors:active) {
      [data-sc-slider-thumb] { background-color:Canvas; border-color:ButtonText; --rail-color:Highlight; forced-color-adjust:none; }
      [data-sc-number-slider][disabled] [data-sc-slider-thumb] { --rail-color:GrayText; }
    }
  `;
}
