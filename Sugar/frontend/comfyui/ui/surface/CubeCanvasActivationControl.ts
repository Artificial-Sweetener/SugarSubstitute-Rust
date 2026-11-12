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
/** Present Cube-owned activation through Comfy's Nodes 1.0 BooleanWidget vocabulary. */

interface ActivationControlRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Reserve enough title-row space for the state label and native boolean dot. */
export const CUBE_CANVAS_ACTIVATION_SIZE = Object.freeze({
  width: 76,
  height: 18,
});

/** Draw the labeled state using the exact active/inactive colors of Comfy's BooleanWidget. */
export function drawCubeCanvasActivationControl(
  context: CanvasRenderingContext2D,
  target: ActivationControlRect,
  enabled: boolean,
): void {
  const centerY = target.y + target.height * 0.5;
  const dotRadius = target.height * 0.36;
  const dotX = target.x + target.width - dotRadius;

  context.save();
  context.font = '12px sans-serif';
  context.textAlign = 'right';
  context.textBaseline = 'middle';
  context.fillStyle = enabled ? '#f0f2f5' : '#9aa1aa';
  context.fillText(enabled ? 'Enabled' : 'Disabled', dotX - dotRadius - 5, centerY);

  context.fillStyle = enabled ? '#89A' : '#333';
  context.beginPath();
  context.arc(dotX, centerY, dotRadius, 0, Math.PI * 2);
  context.fill();
  context.restore();
}
