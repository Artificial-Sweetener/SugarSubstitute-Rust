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
/** Resolve owned-surface brightness from the accepted visual-style palette. */
import { parseCssColor } from '../CssColor.js';
import { readDocumentVisualStyle } from './VisualStyleBoundary.js';

/** Let PhotoPro follow its accepted Photoshop palette; other styles follow the Comfy surface. */
export function readOwnedDarkTheme(document: Document): boolean {
  if (readDocumentVisualStyle(document) === 'photopro')
    return document.documentElement.getAttribute('data-sc-photopro-palette') !== 'light-gray';
  const style = document.defaultView?.getComputedStyle(document.documentElement);
  const background = parseCssColor(style?.getPropertyValue('--comfy-menu-bg') ?? '');
  if (background?.alpha === 1) {
    const luminance =
      linearChannel(background.red) * 0.2126 +
      linearChannel(background.green) * 0.7152 +
      linearChannel(background.blue) * 0.0722;
    return luminance < 0.179;
  }
  const scheme = style?.colorScheme;
  return scheme === 'dark' || (scheme !== 'light' && !document.querySelector('.light-theme'));
}

/** Convert one sRGB channel before comparing dark and light text contrast. */
function linearChannel(channel: number): number {
  const value = channel / 255;
  return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
}
