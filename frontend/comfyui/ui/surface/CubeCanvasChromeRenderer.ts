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
/** Render Nodes 1.0 Cube header chrome with Comfy-owned icon primitives. */

import { resolveCubeIdentityPresentation } from '../cube/CubeIdentityPresentation.js';
import { requireCubeIdentity, type CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { drawFallbackInitialsCanvas } from '../core/CubeFallbackIconRenderer.js';
import { CubeIconResolver, type CubeIconModel } from '../core/CubeIconResolver.js';
import type { CubeCanvasLayout, CubeCanvasRect } from './CubeCanvasLayout.js';
import {
  resolveCubeFaceTitlebarActions,
  type CubeFaceChromeActions,
} from './CubeFaceChromeActions.js';
import { drawComfyPrimeIcon, type ComfyPrimeIconName } from './ComfyPrimeIcons.js';
import { drawCubeUnsavedIndicator } from './CubeUnsavedIndicator.js';
import { CubeModelPillCanvasRenderer } from './CubeModelPillCanvasRenderer.js';

/** Describe the native LiteGraph button retained from the real SubgraphNode. */
export interface NativeLiteGraphTitleButton {
  name?: string;
  height: number;
  visible: boolean;
  xOffset: number;
  yOffset: number;
  getWidth(context: CanvasRenderingContext2D): number;
  draw(context: CanvasRenderingContext2D, x: number, y: number): void;
}

export interface CubeCanvasChromeRenderItem {
  node: CubeNode;
  layout: CubeCanvasLayout;
  chromeActions: CubeFaceChromeActions | null;
  editorButton: NativeLiteGraphTitleButton | null;
  headerColor: string;
  titleTextColor: string;
}

/** Own Cube header composition without owning cards, previews, or interaction. */
export class CubeCanvasChromeRenderer {
  readonly #icons: CubeIconResolver;
  readonly #modelTitles = new CubeModelPillCanvasRenderer();

  /** Bind Cube-definition icon loading to the header renderer. */
  constructor(icons: CubeIconResolver) {
    this.#icons = icons;
  }

  /** Draw one Cube header and every currently available action. */
  draw(context: CanvasRenderingContext2D, item: CubeCanvasChromeRenderItem): void {
    const { node, layout } = item;
    const identity = resolveCubeIdentityPresentation({
      metadata: requireCubeIdentity(node),
      instanceTitle: node.title?.trim() || node.subgraph.name,
      fallbackDefinitionTitle: node.subgraph.name,
    });
    context.fillStyle = item.headerColor;
    context.fillRect(layout.header.x, layout.header.y, layout.header.width, layout.header.height);
    const iconSize = 24;
    const iconX = layout.header.x + 12;
    const iconY = layout.header.y + (layout.header.height - iconSize) / 2;
    this.#drawDefinitionIcon(context, identity.icon, iconX, iconY, iconSize);
    this.#modelTitles.draw(context, identity.instanceModelTitle, {
      x: iconX + iconSize + 7,
      y: layout.header.y + layout.header.height / 2,
      maxWidth: Math.max(1, layout.header.width / 2 - iconSize - 34),
      font: '600 16px sans-serif',
      fontFamily: 'sans-serif',
      fontSize: 16,
      textColor: item.titleTextColor,
      punchoutColor: item.headerColor,
      align: 'left',
    });
    context.save();
    context.textAlign = 'center';
    context.textBaseline = 'middle';
    const definitionCenterX = layout.header.x + layout.header.width / 2;
    const definitionMaxWidth = Math.max(1, layout.editAction.x - definitionCenterX - 8);
    this.#modelTitles.draw(context, identity.definitionModelTitle, {
      x: definitionCenterX,
      y: layout.header.y + layout.header.height / 2 - 6,
      maxWidth: definitionMaxWidth,
      font: '12px sans-serif',
      fontFamily: 'sans-serif',
      fontSize: 12,
      textColor: item.titleTextColor,
      punchoutColor: item.headerColor,
      align: 'center',
    });
    context.font = '10px sans-serif';
    context.fillStyle = '#aeb4bd';
    context.fillText(
      identity.sourceLine,
      definitionCenterX,
      layout.header.y + layout.header.height / 2 + 7,
      definitionMaxWidth,
    );
    context.restore();

    drawNativeEditorButton(context, layout.editAction, item.editorButton);
    if (layout.unsavedIndicator) {
      drawCubeUnsavedIndicator(
        context,
        layout.unsavedIndicator.x + layout.unsavedIndicator.width / 2,
        layout.unsavedIndicator.y + layout.unsavedIndicator.height / 2,
      );
    }
    if (layout.cardMenuEntries.length > 0) {
      drawPrimeIconAction(context, layout.cardMenuAction, 'eye');
    }
    for (const action of resolveCubeFaceTitlebarActions(
      requireCubeIdentity(node),
      item.chromeActions,
    )) {
      const target = layout.chromeActions[action.key];
      if (target) drawPrimeIconAction(context, target, action.icon);
    }
  }

  /** Draw one definition asset or its exact initials fallback. */
  #drawDefinitionIcon(
    context: CanvasRenderingContext2D,
    model: CubeIconModel,
    x: number,
    y: number,
    size: number,
  ): void {
    const entry = this.#icons.getImage(model);
    if (model.kind === 'asset' && entry.status === 'ready' && entry.image) {
      const width = Math.max(1, entry.image.naturalWidth);
      const height = Math.max(1, entry.image.naturalHeight);
      const scale = Math.min(size / width, size / height);
      const targetWidth = width * scale;
      const targetHeight = height * scale;
      context.drawImage(
        entry.image,
        x + (size - targetWidth) / 2,
        y + (size - targetHeight) / 2,
        targetWidth,
        targetHeight,
      );
      return;
    }
    drawFallbackInitialsCanvas(context, model, x, y, size);
  }
}

/** Reuse the real SubgraphNode title button, preserving Comfy's exact icon drawing. */
function drawNativeEditorButton(
  context: CanvasRenderingContext2D,
  target: CubeCanvasRect,
  button: NativeLiteGraphTitleButton | null,
): void {
  if (!button?.visible) {
    context.fillStyle = '#f0f2f5';
    drawComfyPrimeIcon(
      context,
      'window-maximize',
      target.x + target.width / 2,
      target.y + target.height / 2,
    );
    return;
  }
  const width = button.getWidth(context);
  const x = target.x + (target.width - width) / 2 - button.xOffset;
  const y = target.y + (target.height - button.height) / 2 - button.yOffset;
  context.save();
  context.fillStyle = '#f0f2f5';
  button.draw(context, x, y);
  context.restore();
}

/** Draw one icon-only Cube action using Comfy's PrimeIcons font. */
function drawPrimeIconAction(
  context: CanvasRenderingContext2D,
  target: CubeCanvasRect,
  icon: ComfyPrimeIconName,
): void {
  context.fillStyle = '#f0f2f5';
  drawComfyPrimeIcon(context, icon, target.x + target.width / 2, target.y + target.height / 2);
}
