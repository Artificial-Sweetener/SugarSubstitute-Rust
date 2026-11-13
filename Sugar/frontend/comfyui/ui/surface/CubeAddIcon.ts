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
/** Compose Comfy-native cube and plus icons for both Cube renderers. */

import { drawComfyPrimeIcon } from './ComfyPrimeIcons.js';

const STYLE_ID = 'sugarcubes-cube-add-icon-styles';

/** Build the native Lucide composite used by Nodes 2.0 controls and menus. */
export function createCubeAddIconElement(documentRef: Document): HTMLSpanElement {
  ensureCubeAddIconStyles(documentRef);
  const root = documentRef.createElement('span');
  root.className = 'sugarcubes-cube-add-icon';
  root.setAttribute('data-sugarcubes-add-cube-icon', '');
  root.setAttribute('aria-hidden', 'true');
  const cube = documentRef.createElement('i');
  cube.className = 'icon-[lucide--box] sugarcubes-cube-add-icon__cube';
  const badge = documentRef.createElement('span');
  badge.className = 'sugarcubes-cube-add-icon__badge';
  const plus = documentRef.createElement('i');
  plus.className = 'icon-[lucide--plus] sugarcubes-cube-add-icon__plus';
  badge.append(plus);
  root.append(cube, badge);
  return root;
}

/** Install the composite icon's shared theme-aware geometry once per document. */
function ensureCubeAddIconStyles(documentRef: Document): void {
  if (documentRef.getElementById(STYLE_ID)) return;
  const style = documentRef.createElement('style');
  style.id = STYLE_ID;
  style.textContent = `
    .sugarcubes-cube-add-icon { position: relative; display: inline-block;
      width: 1rem; height: 1rem; flex: 0 0 1rem; }
    .sugarcubes-cube-add-icon__cube { position: absolute; inset: 0;
      width: 100%; height: 100%; }
    .sugarcubes-cube-add-icon__badge { position: absolute; right: -0.1875rem;
      bottom: -0.1875rem; display: inline-flex; align-items: center; justify-content: center;
      width: 0.75rem; height: 0.75rem; border-radius: 50%;
      background: var(--comfy-menu-bg, #171b20); }
    .sugarcubes-cube-add-icon__plus { width: 0.625rem; height: 0.625rem; }
  `;
  documentRef.head.append(style);
}

/** Draw the equivalent PrimeIcons composite on the Nodes 1.0 canvas. */
export function drawCubeAddIcon(
  context: CanvasRenderingContext2D,
  centerX: number,
  centerY: number,
  backgroundColor: string,
): void {
  drawComfyPrimeIcon(context, 'box', centerX - 1, centerY - 1, 16);
  context.save();
  context.fillStyle = backgroundColor;
  context.beginPath();
  context.arc(centerX + 6, centerY + 6, 6, 0, Math.PI * 2);
  context.fill();
  context.fillStyle = '#f0f2f5';
  drawComfyPrimeIcon(context, 'plus', centerX + 6, centerY + 6, 10);
  context.restore();
}
