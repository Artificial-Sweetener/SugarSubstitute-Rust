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
/** Present the shared first-save warning on DOM and canvas Cube chrome. */

import { createComfyPrimeIconElement, drawComfyPrimeIcon } from './ComfyPrimeIcons.js';

/** Explain the first-save warning to assistive technology and pointer users. */
export const CUBE_UNSAVED_INDICATOR_LABEL = 'Not saved yet';

/** Build the crossed-out save mark from Comfy's native PrimeIcons. */
export function createCubeUnsavedIndicator(documentRef: Document): HTMLSpanElement {
  const indicator = documentRef.createElement('span');
  indicator.className = 'sugarcubes-cube-unsaved-indicator';
  indicator.setAttribute('role', 'img');
  indicator.setAttribute('aria-label', CUBE_UNSAVED_INDICATOR_LABEL);
  indicator.title = CUBE_UNSAVED_INDICATOR_LABEL;
  const save = createComfyPrimeIconElement(documentRef, 'save');
  save.classList.add('sugarcubes-cube-unsaved-indicator__save');
  const ban = createComfyPrimeIconElement(documentRef, 'ban');
  ban.classList.add('sugarcubes-cube-unsaved-indicator__ban');
  indicator.append(save, ban);
  return indicator;
}

/** Draw the same crossed-out save mark on LiteGraph's canvas titlebar. */
export function drawCubeUnsavedIndicator(
  context: CanvasRenderingContext2D,
  centerX: number,
  centerY: number,
): void {
  context.save();
  context.fillStyle = '#d7dbe2';
  drawComfyPrimeIcon(context, 'save', centerX, centerY, 15);
  context.fillStyle = '#f0b35c';
  drawComfyPrimeIcon(context, 'ban', centerX, centerY, 19);
  context.restore();
}
