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
/** Own the default Cube-face visual treatment independently of layout geometry. */
export function ensureNodeCardAppearanceStyles(documentRef) {
    if (documentRef.getElementById('sugarcubes-node-card-appearance'))
        return;
    const style = documentRef.createElement('style');
    style.id = 'sugarcubes-node-card-appearance';
    style.textContent = `
    .sugarcubes-native-node-card .lg-node:not([data-sc-fluent-node]) [data-testid="node-inner-wrapper"] {
      background-color:var(--sugarcubes-cube-card-header) !important;
    }
    .sugarcubes-native-node-card .lg-node:not([data-sc-fluent-node]) [data-testid^="node-body-"] {
      background-color:var(--sugarcubes-cube-card-body) !important;
    }
  `;
    documentRef.head.append(style);
}
