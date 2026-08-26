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
/** Draw custom Cube faces from one real Nodes 1.0 node lifecycle. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { CubeIconResolver } from '../core/CubeIconResolver.js';
import type { ComfyNode } from '../types/graph.js';
import { drawCubeCanvasActivationControl } from './CubeCanvasActivationControl.js';
import { drawNativeLiteGraphCubeCard } from './ComfyLiteGraphNodeCardRenderer.js';
import type { CubeCanvasCard, CubeCanvasLayout, CubeCanvasRect } from './CubeCanvasLayout.js';
import type { CubeCanvasPreviewImageProvider } from './CubeCanvasPreviewImageCache.js';
import type { CubePreviewSnapshot } from './CubePreviewModel.js';
import { type CubeFaceChromeActions } from './CubeFaceChromeActions.js';
import {
  CubeCanvasChromeRenderer,
  type NativeLiteGraphTitleButton,
} from './CubeCanvasChromeRenderer.js';
import type { CubeIdentitySource } from '../cube/CubeIdentityPresentation.js';
import type { UnknownRecord } from '../types/common.js';
import { drawComfyPrimeIcon } from './ComfyPrimeIcons.js';
import { CubeCanvasPortRenderer } from './CubeCanvasPortRenderer.js';
import {
  deriveCubeBackdropColor,
  resolveCubeNodeColorTheme,
  type CubeNodeColorTheme,
} from './CubeNodeColorTheme.js';
import {
  resolveComfyLiteGraphCubeSurfaceTheme,
  resolveComfyLiteGraphTitleTextColor,
} from './ComfyLiteGraphNodeColorTheme.js';
import {
  CUBE_PREVIEW_TITLE_LINE_HEIGHT,
  layoutCubeCanvasPreviewSections,
} from './CubePreviewSections.js';

export interface LiteGraphCubeDrawHost {
  graph_mouse?: ArrayLike<number>;
  ds?: {
    scale?: number;
    offset?: ArrayLike<number>;
  };
  colourGetter?: unknown;
  editor_alpha?: number;
  inner_text_font?: string;
  drawNode(node: ComfyNode, context: CanvasRenderingContext2D): void;
}

export interface ComfyLiteGraphCubeRenderItem {
  node: CubeNode;
  layout: CubeCanvasLayout;
  preview: CubePreviewSnapshot | null;
  chromeActions: CubeFaceChromeActions | null;
  editorButton: NativeLiteGraphTitleButton | null;
}

/** Own only visual composition for legacy canvas Cube surfaces. */
export class ComfyLiteGraphCubeRenderer {
  readonly #host: LiteGraphCubeDrawHost;
  readonly #previewImages: CubeCanvasPreviewImageProvider;
  readonly #chrome: CubeCanvasChromeRenderer;
  readonly #ports = new CubeCanvasPortRenderer();

  /** Bind Comfy's exact active node and boundary render surfaces. */
  constructor(
    host: LiteGraphCubeDrawHost,
    previewImages: CubeCanvasPreviewImageProvider,
    icons: CubeIconResolver,
    resolveIdentitySource?: (metadata: UnknownRecord) => CubeIdentitySource | null,
  ) {
    this.#host = host;
    this.#previewImages = previewImages;
    this.#chrome = new CubeCanvasChromeRenderer(icons, resolveIdentitySource);
  }

  /** Draw one ordered set of graph-space Cube surfaces. */
  draw(context: CanvasRenderingContext2D, items: readonly ComfyLiteGraphCubeRenderItem[]): void {
    for (const item of items) this.#drawCube(context, item);
  }

  /** Draw one frame, native cards, preview rail, and graph-owned ports. */
  #drawCube(context: CanvasRenderingContext2D, item: ComfyLiteGraphCubeRenderItem): void {
    const { layout } = item;
    const cardTheme = resolveCubeNodeColorTheme(item.node);
    const surfaceTheme = resolveComfyLiteGraphCubeSurfaceTheme(item.node);
    context.save();
    context.beginPath();
    context.roundRect(
      layout.frame.x + 1,
      layout.frame.y + 1,
      Math.max(1, layout.frame.width - 2),
      Math.max(1, layout.frame.height - 2),
      13,
    );
    context.fillStyle = deriveCubeBackdropColor(surfaceTheme.body);
    context.fill();
    context.clip();

    this.#chrome.draw(context, {
      ...item,
      headerColor: surfaceTheme.header,
      titleTextColor: resolveComfyLiteGraphTitleTextColor(),
    });
    for (const card of layout.cards) this.#drawNativeCard(context, card, cardTheme);
    if (layout.preview) this.#drawPreview(context, layout.preview, item);
    this.#ports.draw(context, layout);
    context.restore();
  }

  /** Invoke Comfy's exact Nodes 1.0 draw against the real internal node. */
  #drawNativeCard(
    context: CanvasRenderingContext2D,
    card: CubeCanvasCard,
    theme: CubeNodeColorTheme | null,
  ): void {
    const { node, rect: target, bodyHeight } = card;
    const titleHeight = Math.max(1, target.height - bodyHeight);
    context.save();
    context.beginPath();
    context.rect(target.x, target.y, target.width, target.height);
    context.clip();
    context.translate(target.x, target.y + titleHeight);
    drawNativeLiteGraphCubeCard(this.#host, node, context, {
      presentationWidth: target.width,
      presentationHeight: bodyHeight,
      ...(theme ? { theme } : {}),
    });
    context.restore();
    if (card.activationAction) {
      drawCubeCanvasActivationControl(context, card.activationAction, card.enabled);
    }
  }

  /** Draw every output in equal horizontal sections using the dedicated cache. */
  #drawPreview(
    context: CanvasRenderingContext2D,
    preview: CubeCanvasRect,
    item: ComfyLiteGraphCubeRenderItem,
  ): void {
    context.strokeStyle = 'rgba(220, 225, 235, 0.22)';
    context.lineWidth = 1;
    context.beginPath();
    context.moveTo(preview.x, preview.y);
    context.lineTo(preview.x, preview.y + preview.height);
    context.stroke();
    const outputSections = layoutCubeCanvasPreviewSections(preview, item.preview);
    if (outputSections.length === 0) {
      context.font = '12px sans-serif';
      context.textBaseline = 'top';
      context.fillStyle = '#aeb5c0';
      context.fillText('No preview available', preview.x + 6, preview.y + 6);
      return;
    }
    for (const output of outputSections) {
      const section = output.rect;
      context.font = '14px sans-serif';
      context.textBaseline = 'middle';
      context.fillStyle = '#f0f2f5';
      context.fillText(
        output.canonicalName,
        section.x,
        section.y + CUBE_PREVIEW_TITLE_LINE_HEIGHT / 2,
        Math.max(1, section.width),
      );
      context.textBaseline = 'top';
      if (output.items.length === 0) {
        context.font = '12px sans-serif';
        context.fillStyle = '#aeb5c0';
        context.fillText(
          'No preview available',
          section.x,
          section.y + CUBE_PREVIEW_TITLE_LINE_HEIGHT,
        );
        continue;
      }
      for (const previewItem of output.items) {
        const media = previewItem.item;
        const image = this.#previewImages.get(media.url, media.sourceLocator);
        if (!image) {
          context.font = '12px sans-serif';
          context.fillStyle = '#aeb5c0';
          context.fillText('Loading output…', previewItem.rect.x, previewItem.rect.y);
          continue;
        }
        const target = fitCubePreviewImage(
          image.naturalWidth,
          image.naturalHeight,
          previewItem.rect.x,
          previewItem.rect.y,
          previewItem.rect.width,
          previewItem.rect.height,
        );
        context.drawImage(image, target.x, target.y, target.width, target.height);
        if (containsPoint(previewItem.rect, this.#host.graph_mouse)) {
          drawPreviewDownloadAction(context, previewItem.downloadAction);
        }
      }
    }
  }
}

/** Draw Comfy's compact hover download affordance over one canvas preview. */
function drawPreviewDownloadAction(
  context: CanvasRenderingContext2D,
  action: CubeCanvasRect,
): void {
  context.save();
  context.fillStyle = '#f2f2f2';
  context.beginPath();
  context.roundRect(action.x, action.y, action.width, action.height, 6);
  context.fill();
  context.fillStyle = '#151515';
  drawComfyPrimeIcon(
    context,
    'download',
    action.x + action.width / 2,
    action.y + action.height / 2,
    14,
  );
  context.restore();
}

/** Return whether the current graph pointer is inside one finite rectangle. */
function containsPoint(area: CubeCanvasRect, point: ArrayLike<number> | undefined): boolean {
  const x = Number(point?.[0]);
  const y = Number(point?.[1]);
  return (
    Number.isFinite(x) &&
    Number.isFinite(y) &&
    x >= area.x &&
    x <= area.x + area.width &&
    y >= area.y &&
    y <= area.y + area.height
  );
}

/** Fit one complete image inside the rail with centered horizontal and top vertical alignment. */
export function fitCubePreviewImage(
  sourceWidth: number,
  sourceHeight: number,
  x: number,
  y: number,
  width: number,
  height: number,
): CubeCanvasRect {
  const safeSourceWidth = Math.max(1, sourceWidth);
  const safeSourceHeight = Math.max(1, sourceHeight);
  const scale = Math.min(width / safeSourceWidth, height / safeSourceHeight);
  const targetWidth = Math.max(1, safeSourceWidth * scale);
  const targetHeight = Math.max(1, safeSourceHeight * scale);
  return {
    x: x + (width - targetWidth) / 2,
    y,
    width: targetWidth,
    height: targetHeight,
  };
}
