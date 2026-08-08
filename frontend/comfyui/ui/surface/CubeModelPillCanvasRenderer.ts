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
/** Measure and draw compact Cube model pills in Nodes 1.0 Canvas headers. */

import type { CubeModelTitlePresentation } from '../cube/CubeModelTitlePresentation.js';

export interface CubeModelPillCanvasOptions {
  x: number;
  y: number;
  maxWidth: number;
  font: string;
  fontFamily: string;
  fontSize: number;
  textColor: string;
  punchoutColor: string;
  align: 'left' | 'center';
}

export interface CubeModelPillCanvasGeometry {
  pillHeight: number;
  renderedWidth: number;
  startX: number;
}

/** Draw one renderer-neutral title model with bounded Canvas geometry. */
export class CubeModelPillCanvasRenderer {
  /** Render a compact pill without changing the caller's Canvas state. */
  draw(
    context: CanvasRenderingContext2D,
    presentation: CubeModelTitlePresentation,
    options: CubeModelPillCanvasOptions,
  ): CubeModelPillCanvasGeometry {
    const maxWidth = Math.max(1, options.maxWidth);
    const visibleText = [presentation.nameText, presentation.suffixText].filter(Boolean).join(' ');
    context.save();
    context.textAlign = 'left';
    context.textBaseline = 'middle';
    context.font = options.font;
    const textWidth = context.measureText(visibleText).width;
    if (!presentation.usesModelPill) {
      const renderedWidth = Math.min(maxWidth, textWidth);
      const startX = resolveStartX(options.x, renderedWidth, options.align);
      context.fillStyle = options.textColor;
      context.fillText(visibleText, startX, options.y, maxWidth);
      context.restore();
      return { pillHeight: 0, renderedWidth, startX };
    }

    const pillFontSize = options.fontSize * 0.78;
    const pillHeight = options.fontSize * 0.94;
    const horizontalPadding = options.fontSize * 0.3;
    const gap = options.fontSize * 0.3;
    context.font = `700 ${String(pillFontSize)}px ${options.fontFamily}`;
    const naturalPillWidth =
      context.measureText(presentation.modelText).width + horizontalPadding * 2;
    const pillWidth = Math.min(naturalPillWidth, maxWidth * 0.45);
    const availableTextWidth = Math.max(1, maxWidth - pillWidth - gap);
    const renderedTextWidth = Math.min(textWidth, availableTextWidth);
    const renderedWidth = Math.min(maxWidth, pillWidth + gap + renderedTextWidth);
    const startX = resolveStartX(options.x, renderedWidth, options.align);
    const pillY = options.y - pillHeight / 2;

    context.beginPath();
    context.roundRect(startX, pillY, pillWidth, pillHeight, pillHeight / 2);
    context.fillStyle = options.textColor;
    context.fill();
    context.fillStyle = options.punchoutColor;
    context.fillText(
      presentation.modelText,
      startX + horizontalPadding,
      options.y,
      Math.max(1, pillWidth - horizontalPadding * 2),
    );
    context.font = options.font;
    context.fillStyle = options.textColor;
    context.fillText(visibleText, startX + pillWidth + gap, options.y, availableTextWidth);
    context.restore();
    return { pillHeight, renderedWidth, startX };
  }
}

/** Resolve a left edge from the caller's semantic alignment point. */
function resolveStartX(x: number, width: number, align: 'left' | 'center'): number {
  return align === 'center' ? x - width / 2 : x;
}
