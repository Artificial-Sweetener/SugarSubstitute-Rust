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
/** Own per-style scrollbar geometry independently of native scrolling and DOM discovery. */
import { readOwnedVisualStyle } from './VisualStyleBoundary.js';
import { PHOTOPRO_SCROLLBAR_WIDTH } from './ScrollbarGeometry.js';

export interface ScrollbarPresentation {
  readonly kind: 'overlay' | 'photopro' | 'gimp';
  readonly width: number;
  readonly horizontalGutter: number;
  readonly verticalGutter: number;
  readonly arrows: boolean;
  readonly reserveCorner: boolean;
  readonly preserveOwnerExtent: boolean;
  readonly context: string;
}

/** Keep width, reserved space and clipping policy consistent for the thumb and its scroll owner. */
export function scrollbarPresentation(document: Document): ScrollbarPresentation {
  const style = readOwnedVisualStyle(document);
  const kind = style === 'photopro' || style === 'gimp' ? style : 'overlay';
  const width = kind === 'photopro' ? PHOTOPRO_SCROLLBAR_WIDTH : 12;
  return {
    kind,
    width,
    horizontalGutter: kind === 'gimp' ? width : 0,
    verticalGutter: kind === 'overlay' ? 0 : width,
    arrows: kind === 'photopro',
    reserveCorner: kind === 'gimp',
    preserveOwnerExtent: kind !== 'overlay',
    context: `${style}:${String(document.documentElement.hasAttribute('data-sc-compact-masonry'))}`,
  };
}
