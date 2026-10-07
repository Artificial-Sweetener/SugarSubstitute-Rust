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
/** Present Substitute's animated inner dot through the official Fluent slider thumb slot. */

/** Supply a decorative thumb while Fluent retains focus, pointer gestures, and value ownership. */
export function createFluentSliderThumb(documentRef: Document): HTMLElement {
  const thumb = documentRef.createElement('span');
  thumb.slot = 'thumb';
  thumb.dataset.scSliderThumb = '';
  thumb.setAttribute('aria-hidden', 'true');
  const center = documentRef.createElement('span');
  center.dataset.scSliderThumbCenter = '';
  thumb.append(center);
  return thumb;
}

/** Animate only the inner dot so hover and press never move the track or pointer hit target. */
export function fluentSliderThumbCss(): string {
  return `
    [data-sc-slider-thumb] {
      display:grid; place-items:center; box-sizing:border-box;
      width:var(--thumb-size); height:var(--thumb-size); border-radius:50%;
      border:1px solid var(--colorNeutralStroke1); background:var(--colorNeutralBackground1);
    }
    [data-sc-slider-thumb-center] {
      display:block; width:10px; height:10px; border-radius:50%; pointer-events:none;
      background:var(--rail-color); transform:scale(1);
      transition:transform var(--durationFaster,100ms) var(--curveEasyEase);
    }
    [data-sc-number-slider]:not([disabled]) [data-sc-slider-thumb]:hover [data-sc-slider-thumb-center] { transform:scale(1.2); }
    [data-sc-number-slider]:not([disabled]):active [data-sc-slider-thumb] [data-sc-slider-thumb-center] { transform:scale(.8); }
    @media (prefers-reduced-motion:reduce) { [data-sc-slider-thumb-center] { transition:none; } }
    @media (forced-colors:active) {
      [data-sc-slider-thumb] { background:Canvas; border-color:ButtonText; forced-color-adjust:none; }
      [data-sc-slider-thumb-center] { background:Highlight; }
      [data-sc-number-slider][disabled] [data-sc-slider-thumb-center] { background:GrayText; }
    }
  `;
}
