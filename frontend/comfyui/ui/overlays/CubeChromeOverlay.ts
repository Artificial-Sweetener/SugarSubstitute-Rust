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
 * Coordinate Cube chrome lifecycle, rendering, and interaction owners.
 */

import { CubeChromeGroupTitleHook } from './CubeChromeGroupTitleHook.js';
import { CubeChromeInteraction } from './CubeChromeInteraction.js';
import { CubeChromePainter } from './CubeChromePainter.js';
import { CubeChromeRenderer } from './CubeChromeRenderer.js';
import type { CubeIconModel } from '../core/CubeIconResolver.js';
import type { ComfyGroup } from '../types/graph.js';
import type {
  ChromeActions,
  ChromeCanvas,
  ChromeDebugState,
  ChromeInstanceState,
  ChromeMetadata,
  ChromeOverlayOptions,
} from './CubeChromeContracts.js';
import type { Vec2 } from '../types/common.js';

export type {
  BadgeRegion,
  BadgeSource,
  ChromeActions,
  ChromeDebugState,
  ChromeMetadata,
  HitRegion,
} from './CubeChromeContracts.js';
type GroupDraw = (this: ComfyGroup, ...args: unknown[]) => unknown;

/** Preserve the host-facing Cube chrome API across focused owners. */
export class CubeChromeOverlay {
  private actions: ChromeActions;
  private readonly painter: CubeChromePainter;
  private readonly renderer: CubeChromeRenderer;
  private readonly interaction: CubeChromeInteraction;
  private readonly groupTitleHook: CubeChromeGroupTitleHook;
  private readonly adapter: ChromeOverlayOptions['adapter'];

  constructor({ adapter = null, actions = {}, resolveSource = null }: ChromeOverlayOptions = {}) {
    this.adapter = adapter;
    this.actions = actions || {};
    this.painter = new CubeChromePainter(
      () => this.requestRedraw(),
      (ctx, x, y, size, color) => this.drawCubeIcon(ctx, x, y, size, color),
    );
    this.renderer = new CubeChromeRenderer({
      getActions: () => this.actions,
      getHoverState: () => this.interaction.getHoverState(),
      resolveSource: typeof resolveSource === 'function' ? resolveSource : null,
      requestRedraw: () => this.requestRedraw(),
      painter: this.painter,
    });
    this.interaction = new CubeChromeInteraction({
      getActions: () => this.actions,
      getHitRegions: () => this.renderer.hitRegions,
      getBadgeRegions: () => this.renderer.badgeRegions,
      requestRedraw: () => this.requestRedraw(),
    });
    this.groupTitleHook = new CubeChromeGroupTitleHook(adapter, this.painter);
  }
  get lastTransform(): DOMMatrix | null {
    return this.renderer.lastTransform;
  }
  buildMenuOptions(options: {
    metadata: ChromeMetadata;
    isDirty?: boolean;
    flavors?: unknown;
  }): Array<{ title: string; callback: () => void }> {
    return this.interaction.buildMenuOptions(options);
  }
  setup(): void {
    this.groupTitleHook.installGroupTitleIconRenderer();
  }
  dispose(): void {
    this.renderer.clear();
    this.interaction.clear();
    this.groupTitleHook.releaseGroupTitleIconRenderer();
  }
  setActions(actions: ChromeActions): void {
    this.actions = actions || {};
  }
  getDebugState(): ChromeDebugState {
    const hover = this.interaction.getHoverState();
    return {
      actions: this.actions,
      hitRegions: this.renderer.hitRegions,
      badgeRegions: this.renderer.badgeRegions,
      hoveredKey: hover.hoveredKey,
      hoveredBadgeInstance: hover.hoveredBadgeInstance,
      hoveredBadgeKey: hover.hoveredBadgeKey,
    };
  }
  installGroupTitleIconRenderer(): void {
    this.groupTitleHook.installGroupTitleIconRenderer();
  }
  releaseGroupTitleIconRenderer(): void {
    this.groupTitleHook.releaseGroupTitleIconRenderer();
  }
  shouldDrawGroupTitleIcon(group: ComfyGroup): boolean {
    return this.groupTitleHook.shouldDrawGroupTitleIcon(group);
  }
  drawManagedGroupWithTitleIcon(
    group: ComfyGroup,
    originalDraw: GroupDraw,
    args: unknown[],
  ): unknown {
    return this.groupTitleHook.drawManagedGroupWithTitleIcon(group, originalDraw, args);
  }
  drawGroupTitleIcon(
    ctx: CanvasRenderingContext2D,
    group: ComfyGroup,
    canvasInstance: ChromeCanvas,
  ): void {
    this.painter.drawGroupTitleIcon(ctx, group, canvasInstance);
  }
  render(ctx: CanvasRenderingContext2D, canvasInstance: ChromeCanvas): void {
    this.renderer.render(ctx, canvasInstance);
  }
  renderHeader(
    ctx: CanvasRenderingContext2D,
    bounds: number[],
    metadata: ChromeMetadata,
    group: ComfyGroup = {},
  ): void {
    this.renderer.renderHeader(ctx, bounds, metadata, group);
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
    this.painter.drawPill(ctx, x, y, w, h, style, options);
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
    this.painter.drawIcon(ctx, icon, x, y, size, color, options);
  }
  drawPenIcon(
    ctx: CanvasRenderingContext2D,
    x: number,
    y: number,
    size: number,
    color: string,
  ): void {
    this.painter.drawPenIcon(ctx, x, y, size, color);
  }
  drawCubeIcon(
    ctx: CanvasRenderingContext2D,
    x: number,
    y: number,
    size: number,
    color: string,
  ): void {
    this.painter.drawCubeIcon(ctx, x, y, size, color);
  }
  drawDefinitionIcon(
    ctx: CanvasRenderingContext2D,
    model: CubeIconModel,
    x: number,
    y: number,
    size: number,
    options: { alpha?: number } = {},
  ): void {
    this.painter.drawDefinitionIcon(ctx, model, x, y, size, options);
  }
  drawContainedImage(
    ctx: CanvasRenderingContext2D,
    image: HTMLImageElement,
    x: number,
    y: number,
    size: number,
  ): void {
    this.painter.drawContainedImage(ctx, image, x, y, size);
  }
  handleMouseDown(event: MouseEvent, canvasInstance: ChromeCanvas): boolean {
    return this.interaction.handleMouseDown(event, canvasInstance);
  }
  handlePointerMove(event: PointerEvent | MouseEvent, canvasInstance: ChromeCanvas): boolean {
    return this.interaction.handlePointerMove(event, canvasInstance);
  }
  drawTooltip(
    ctx: CanvasRenderingContext2D,
    centerX: number,
    topY: number,
    text: string,
    fontSize: number,
  ): void {
    this.painter.drawTooltip(ctx, centerX, topY, text, fontSize);
  }
  isAnimating(state: ChromeInstanceState | null, now: number): boolean {
    return this.renderer.isAnimating(state, now);
  }
  ensureInstanceState(instanceId: string): ChromeInstanceState {
    return this.renderer.ensureInstanceState(instanceId);
  }
  isHovered(instanceId: string, key: string): boolean {
    return this.renderer.isHovered(instanceId, key);
  }
  requestRedraw(): void {
    this.adapter?.getApp?.()?.canvas?.setDirty?.(true, true);
  }
  convertEventToCanvasPoint(
    event: MouseEvent | PointerEvent,
    canvasInstance: ChromeCanvas,
  ): Vec2 | null {
    return this.interaction.convertEventToCanvasPoint(event, canvasInstance);
  }
  convertCanvasPoint(canvasInstance: ChromeCanvas, point: Vec2): Vec2 | null {
    return this.interaction.convertCanvasPoint(canvasInstance, point);
  }
}
