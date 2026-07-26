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
/** Draw Cube input labels and output-to-preview association leaders. */

import type { CubeCanvasLayout } from './CubeCanvasLayout.js';
import { resolveCubeCanvasOutputSocketRimX } from './CubeCanvasBoundaryGeometry.js';
import { resolveCubeOutputLeader, type CubeLeaderPoint } from './CubeOutputLeaderGeometry.js';
export { resolveCubeOutputLeader } from './CubeOutputLeaderGeometry.js';

const INPUT_LABEL_INSET = 16;

/** Draw presentation-only boundary context without impersonating graph noodles. */
export class CubeCanvasPortRenderer {
  /** Draw elided input types and orthogonal output association leaders. */
  draw(context: CanvasRenderingContext2D, layout: CubeCanvasLayout): void {
    this.#drawInputs(context, layout);
    this.#drawOutputLeaders(context, layout);
  }

  /** Draw input type labels entirely inside the dedicated left gutter. */
  #drawInputs(context: CanvasRenderingContext2D, layout: CubeCanvasLayout): void {
    const maximumWidth = Math.max(1, layout.inputGutter.width - INPUT_LABEL_INSET - 8);
    context.save();
    context.font = '12px sans-serif';
    context.textBaseline = 'middle';
    context.fillStyle = '#c8ced8';
    for (const port of layout.inputs) {
      const label = elideCubePortLabel(
        port.type,
        maximumWidth,
        (value) => context.measureText(value).width,
      );
      context.fillText(label, layout.frame.x + INPUT_LABEL_INSET, port.y, maximumWidth);
    }
    context.restore();
  }

  /** Connect each fixed preview title to its independently moving output socket. */
  #drawOutputLeaders(context: CanvasRenderingContext2D, layout: CubeCanvasLayout): void {
    if (!layout.preview) return;
    context.save();
    context.strokeStyle = 'rgba(200, 206, 216, 0.62)';
    context.lineWidth = 1.25;
    context.lineJoin = 'round';
    context.font = '14px sans-serif';
    for (const port of layout.outputs) {
      const titleX = layout.preview.x + 6;
      const labelEndX = Math.min(
        layout.outputGutter.x - 8,
        titleX + context.measureText(port.name).width + 8,
      );
      const socketX = resolveCubeCanvasOutputSocketRimX(layout.frame.x + layout.frame.width);
      const elbowX = layout.outputGutter.x + 8;
      const points = resolveCubeOutputLeader(labelEndX, port.labelY, socketX, port.y, elbowX);
      strokePolyline(context, points);
    }
    context.restore();
  }
}

/** Elide one boundary label to a measured gutter width. */
export function elideCubePortLabel(
  label: string,
  maximumWidth: number,
  measure: (value: string) => number,
): string {
  const normalized = label.trim() || '*';
  if (measure(normalized) <= maximumWidth) return normalized;
  const ellipsis = '…';
  let retained = normalized;
  while (retained && measure(`${retained}${ellipsis}`) > maximumWidth) {
    retained = retained.slice(0, -1);
  }
  return retained ? `${retained}${ellipsis}` : ellipsis;
}

/** Stroke one small point sequence without allocating a Path2D. */
function strokePolyline(
  context: CanvasRenderingContext2D,
  points: readonly CubeLeaderPoint[],
): void {
  const first = points[0];
  if (!first) return;
  context.beginPath();
  context.moveTo(first[0], first[1]);
  for (const point of points.slice(1)) context.lineTo(point[0], point[1]);
  context.stroke();
}
