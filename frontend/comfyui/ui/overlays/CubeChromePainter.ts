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
/**
 * Paint Cube chrome icons, pills, tooltips, and managed group titles.
 */

import { CubeIconResolver } from '../core/CubeIconResolver.js';
import { drawFallbackInitialsCanvas } from '../core/CubeFallbackIconRenderer.js';
import {
  resolveCubeDisplayName,
  resolveInstanceDisplayName,
  getGroupSugarcubes,
} from '../graph/GroupMetadata.js';
import { readGroupBounds } from '../graph/Bounds.js';
import {
  clampNumber,
  resolveGroupFontFamily,
  resolveGroupTitleColor,
  resolveGroupTitlePadding,
} from './CubeChromeLayout.js';
import type { CubeIconModel } from '../core/CubeIconResolver.js';
import type { ComfyGroup } from '../types/graph.js';
import type { ChromeCanvas } from './CubeChromeContracts.js';

/** Paint reusable Cube chrome canvas primitives. */
export class CubeChromePainter {
  private readonly iconResolver: CubeIconResolver;

  constructor(
    requestRedraw: () => void,
    private readonly paintCubeIcon: (
      ctx: CanvasRenderingContext2D,
      x: number,
      y: number,
      size: number,
      color: string,
    ) => void,
  ) {
    this.iconResolver = new CubeIconResolver({ onImageLoad: requestRedraw });
  }

  drawGroupTitleIcon(
    ctx: CanvasRenderingContext2D,
    group: ComfyGroup,
    canvasInstance: ChromeCanvas,
  ): void {
    const metadata = getGroupSugarcubes(group);
    const bounds = readGroupBounds(group);
    if (!metadata || !bounds) {
      return;
    }
    const [x, y] = bounds;
    const fontSize = Math.max(14, Number(group?.font_size) || Number(metadata?.font_size) || 18);
    const fontFamily = resolveGroupFontFamily();
    const padding = resolveGroupTitlePadding();
    const headerHeight = fontSize * 1.4;
    const iconSize = clampNumber(Math.floor(headerHeight - 8), 18, 28);
    const iconGap = 7;
    const title = resolveInstanceDisplayName({
      metadata,
      group,
      fallback: resolveCubeDisplayName({ metadata, group, fallback: 'SugarCube' }),
    });
    const iconModel = this.iconResolver.resolve({
      icon: metadata?.icon,
      cube_id: metadata?.cube_id,
      default_alias: metadata?.default_alias,
    });
    const baseAlpha = Number.isFinite(ctx.globalAlpha) ? ctx.globalAlpha : 1;
    const iconX = x + padding;
    const iconY = y + Math.max(2, (headerHeight - iconSize) / 2);

    this.drawDefinitionIcon(ctx, iconModel, iconX, iconY, iconSize, {
      alpha: baseAlpha * 0.96,
    });

    if (!title) {
      return;
    }
    ctx.save();
    ctx.globalAlpha = Math.max(0, Math.min(1, canvasInstance?.editor_alpha ?? baseAlpha));
    ctx.font = `${fontSize}px ${fontFamily}`;
    ctx.fillStyle = resolveGroupTitleColor(group);
    ctx.textAlign = 'left';
    ctx.textBaseline = 'alphabetic';
    ctx.fillText(title, iconX + iconSize + iconGap, y + fontSize);
    ctx.restore();
  }

  drawPill(
    ctx: CanvasRenderingContext2D,
    x: number,
    y: number,
    w: number,
    h: number,
    style: string | undefined,
    options: { hovered?: boolean; pulse?: number; alpha?: number } = {},
  ): void {
    const radius = Math.min(10, h / 2);
    const hovered = Boolean(options.hovered);
    const pulse = Number(options.pulse) || 0;
    const alpha = Number.isFinite(options.alpha) ? Number(options.alpha) : 1;
    const shadowBoost = hovered ? 8 : 0;
    const pulseBoost = pulse ? 10 * pulse : 0;
    ctx.save();
    ctx.shadowColor = 'rgba(80, 180, 255, 0.35)';
    ctx.shadowBlur = shadowBoost + pulseBoost;
    ctx.shadowOffsetY = hovered ? 2 : 0;
    ctx.globalAlpha = Math.max(0, Math.min(1, alpha));
    ctx.beginPath();
    ctx.moveTo(x + radius, y);
    ctx.lineTo(x + w - radius, y);
    ctx.arcTo(x + w, y, x + w, y + radius, radius);
    ctx.lineTo(x + w, y + h - radius);
    ctx.arcTo(x + w, y + h, x + w - radius, y + h, radius);
    ctx.lineTo(x + radius, y + h);
    ctx.arcTo(x, y + h, x, y + h - radius, radius);
    ctx.lineTo(x, y + radius);
    ctx.arcTo(x, y, x + radius, y, radius);
    ctx.closePath();
    if (style === 'danger') {
      ctx.fillStyle = '#d3514a';
      ctx.strokeStyle = 'rgba(0, 0, 0, 0.35)';
    } else if (style === 'badge') {
      ctx.fillStyle = 'rgba(18, 32, 40, 0.6)';
      ctx.strokeStyle = 'rgba(255, 255, 255, 0.12)';
    } else {
      ctx.fillStyle = hovered ? 'rgba(26, 50, 64, 0.85)' : 'rgba(18, 32, 40, 0.72)';
      ctx.strokeStyle = hovered ? 'rgba(120, 200, 255, 0.4)' : 'rgba(255, 255, 255, 0.15)';
    }
    ctx.lineWidth = 1;
    ctx.fill();
    ctx.stroke();
    ctx.restore();
  }

  drawIcon(
    ctx: CanvasRenderingContext2D,
    icon: string,
    x: number,
    y: number,
    size: number,
    color: string | undefined,
    options: { now?: number; alpha?: number } = {},
  ): void {
    const alpha = Number.isFinite(options.alpha) ? Number(options.alpha) : 1;
    ctx.save();
    ctx.globalAlpha = Math.max(0, Math.min(1, alpha));
    if (icon === 'pen') {
      this.drawPenIcon(ctx, x, y, size, color || '#f3f7fb');
      ctx.restore();
      return;
    }
    if (icon === 'cube') {
      this.paintCubeIcon(ctx, x, y, size, color || '#f3f7fb');
      ctx.restore();
      return;
    }
    ctx.restore();
  }

  drawPenIcon(
    ctx: CanvasRenderingContext2D,
    x: number,
    y: number,
    size: number,
    color: string,
  ): void {
    const thickness = Math.max(2, size * 0.18);
    ctx.save();
    ctx.translate(x + size * 0.5, y + size * 0.5);
    ctx.rotate(-Math.PI / 4);
    ctx.fillStyle = color;
    ctx.strokeStyle = color;
    ctx.lineWidth = thickness;
    ctx.beginPath();
    ctx.moveTo(-size * 0.35, 0);
    ctx.lineTo(size * 0.25, 0);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(size * 0.25, -size * 0.12);
    ctx.lineTo(size * 0.42, 0);
    ctx.lineTo(size * 0.25, size * 0.12);
    ctx.closePath();
    ctx.fill();
    ctx.restore();
  }

  drawCubeIcon(
    ctx: CanvasRenderingContext2D,
    x: number,
    y: number,
    size: number,
    color: string,
  ): void {
    const stroke = Math.max(1.2, size * 0.1);
    const inset = size * 0.18;
    const offset = size * 0.16;
    const front = {
      x: x + inset,
      y: y + inset + offset,
      w: size - inset * 2 - offset,
      h: size - inset * 2 - offset,
    };
    ctx.save();
    ctx.strokeStyle = color;
    ctx.lineWidth = stroke;
    ctx.beginPath();
    ctx.rect(front.x, front.y, front.w, front.h);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(front.x, front.y);
    ctx.lineTo(front.x + offset, front.y - offset);
    ctx.lineTo(front.x + front.w + offset, front.y - offset);
    ctx.lineTo(front.x + front.w, front.y);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(front.x + front.w, front.y);
    ctx.lineTo(front.x + front.w + offset, front.y - offset);
    ctx.lineTo(front.x + front.w + offset, front.y + front.h - offset);
    ctx.lineTo(front.x + front.w, front.y + front.h);
    ctx.stroke();
    ctx.restore();
  }

  drawDefinitionIcon(
    ctx: CanvasRenderingContext2D,
    model: CubeIconModel,
    x: number,
    y: number,
    size: number,
    options: { alpha?: number } = {},
  ): void {
    const alpha = Number.isFinite(options.alpha) ? Number(options.alpha) : 1;
    ctx.save();
    ctx.globalAlpha = Math.max(0, Math.min(1, alpha));

    if (model?.kind === 'asset') {
      const entry = this.iconResolver.getImage(model);
      if (entry.status === 'ready' && entry.image) {
        this.drawContainedImage(ctx, entry.image, x, y, size);
        ctx.restore();
        return;
      }
    }

    if (model?.kind === 'initials' || model?.initials) {
      drawFallbackInitialsCanvas(ctx, model, x, y, size);
      ctx.restore();
      return;
    }

    this.paintCubeIcon(ctx, x + size * 0.16, y + size * 0.14, size * 0.72, '#ffffff');
    ctx.restore();
  }

  drawContainedImage(
    ctx: CanvasRenderingContext2D,
    image: HTMLImageElement,
    x: number,
    y: number,
    size: number,
  ): void {
    if (typeof ctx.drawImage !== 'function') {
      return;
    }
    const width = Number(image?.naturalWidth || image?.width) || size;
    const height = Number(image?.naturalHeight || image?.height) || size;
    const scale = Math.min(size / width, size / height);
    const drawWidth = width * scale;
    const drawHeight = height * scale;
    const drawX = x + (size - drawWidth) / 2;
    const drawY = y + (size - drawHeight) / 2;
    ctx.drawImage(image, drawX, drawY, drawWidth, drawHeight);
  }

  drawTooltip(
    ctx: CanvasRenderingContext2D,
    centerX: number,
    topY: number,
    text: string,
    fontSize: number,
  ): void {
    if (!text) {
      return;
    }
    const paddingX = 6;
    const paddingY = 4;
    const tooltipFont = Math.max(10, fontSize - 2);
    ctx.save();
    ctx.font = `${tooltipFont}px sans-serif`;
    const textWidth = ctx.measureText(text).width;
    const width = textWidth + paddingX * 2;
    const height = tooltipFont + paddingY * 2;
    const x = centerX - width / 2;
    const y = topY - height - 6;
    ctx.fillStyle = 'rgba(15, 22, 28, 0.9)';
    ctx.strokeStyle = 'rgba(120, 180, 220, 0.4)';
    ctx.lineWidth = 1;
    const radius = Math.min(6, height / 2);
    ctx.beginPath();
    ctx.moveTo(x + radius, y);
    ctx.lineTo(x + width - radius, y);
    ctx.arcTo(x + width, y, x + width, y + radius, radius);
    ctx.lineTo(x + width, y + height - radius);
    ctx.arcTo(x + width, y + height, x + width - radius, y + height, radius);
    ctx.lineTo(x + radius, y + height);
    ctx.arcTo(x, y + height, x, y + height - radius, radius);
    ctx.lineTo(x, y + radius);
    ctx.arcTo(x, y, x + radius, y, radius);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
    ctx.fillStyle = '#f3f7fb';
    ctx.textBaseline = 'middle';
    ctx.fillText(text, x + paddingX, y + height / 2);
    ctx.restore();
  }
}
