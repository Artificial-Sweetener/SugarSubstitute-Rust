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
/** Present licensed Microsoft action glyphs without parsing SVG into live document markup. */
const actionIcons = {
  globe: new URL('../../../icons/model/globe.svg', import.meta.url).href,
  download: new URL('../../../icons/model/download.svg', import.meta.url).href,
  key: new URL('../../../icons/model/key.svg', import.meta.url).href,
};

/** Return a decorative foreground-colored glyph for an already labeled action. */
export function modelActionIcon(
  documentRef: Document,
  action: keyof typeof actionIcons,
): HTMLElement {
  const icon = documentRef.createElement('span');
  icon.className = 'sc-model-action-icon';
  icon.setAttribute('aria-hidden', 'true');
  icon.style.setProperty('--sc-model-action-icon', `url("${actionIcons[action]}")`);
  return icon;
}
