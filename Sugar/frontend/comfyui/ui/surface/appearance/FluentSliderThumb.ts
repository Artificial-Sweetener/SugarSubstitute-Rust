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
export function createFluentSliderThumb(documentRef: Document): HTMLElement {
  const thumb = documentRef.createElement('span');
  thumb.slot = 'thumb';
  thumb.dataset.scSliderThumb = '';
  thumb.setAttribute('aria-hidden', 'true');
  const svgNamespace = 'http://www.w3.org/2000/svg';
  const surface = documentRef.createElementNS(svgNamespace, 'svg');
  surface.setAttribute('viewBox', '0 0 20 20');
  surface.setAttribute('focusable', 'false');
  const ring = documentRef.createElementNS(svgNamespace, 'circle');
  ring.dataset.scSliderRing = '';
  ring.setAttribute('cx', '10');
  ring.setAttribute('cy', '10');
  ring.setAttribute('r', '9.5');
  const dot = documentRef.createElementNS(svgNamespace, 'circle');
  dot.dataset.scSliderDot = '';
  dot.setAttribute('cx', '10');
  dot.setAttribute('cy', '10');
  dot.setAttribute('r', '5');
  surface.append(ring, dot);
  thumb.append(surface);
  return thumb;
}

/** Keep both circles in one vector coordinate system under fractional graph zoom. */
export function fluentSliderThumbCss(): string {
  return `
    [data-sc-slider-thumb] {
      display:block; box-sizing:border-box;
      width:var(--thumb-size); height:var(--thumb-size);
    }
    [data-sc-slider-thumb] > svg { display:block; width:100%; height:100%; }
    [data-sc-slider-ring] { fill:var(--sc-surface); stroke:var(--sc-border); stroke-width:1px; }
    [data-sc-slider-dot] { fill:var(--rail-color); r:5px; transition:r var(--sc-motion-faster,100ms) var(--sc-motion-ease); }
    [data-sc-number-slider]:not([disabled]) [data-sc-slider-thumb]:hover [data-sc-slider-dot] { r:6px; }
    [data-sc-number-slider]:not([disabled]):active [data-sc-slider-thumb] [data-sc-slider-dot] { r:4px; }
    @media (prefers-reduced-motion:reduce) { [data-sc-slider-dot] { transition:none; } }
    @media (forced-colors:active) {
      [data-sc-slider-thumb] { --sc-surface:Canvas; --sc-border:ButtonText; --rail-color:Highlight; forced-color-adjust:none; }
      [data-sc-number-slider][disabled] [data-sc-slider-thumb] { --rail-color:GrayText; }
    }
  `;
}
