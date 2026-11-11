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
 * Render Cube header chrome and publish immutable interaction regions.
 */

import { getGraphGroups } from '../graph/GraphQuery.js';
import {
  getGroupSugarcubes,
  resolveCubeDisplayName,
  resolveInstanceDisplayName,
} from '../graph/GroupMetadata.js';
import { readGroupBounds } from '../graph/Bounds.js';
import { formatCubeSourceText, formatCubeVersionText } from '../cube/CubeIdentityPresentation.js';
import {
  CHROME_BADGE_MAX_WIDTH,
  CHROME_BADGE_MIN_WIDTH,
  CHROME_BUTTONS,
  buildStackedBadgeLayout,
  clampNumber,
  computeCenteredBadgeSlot,
  computePillLayout,
  hasSwapEligibility,
  removeUnpinnedPillEntry,
  resolveGroupFontFamily,
  resolveGroupTitleColor,
  resolveGroupTitlePadding,
  sumPillWidths,
  triadicColor,
  type ChromeItem,
} from './CubeChromeLayout.js';
import { CubeChromePainter } from './CubeChromePainter.js';
import { CubeChromeHeaderContentPainter } from './CubeChromeHeaderContentPainter.js';
import { CubeChromeAnimationState } from './CubeChromeAnimationState.js';
import type { ComfyGroup } from '../types/graph.js';
import type {
  BadgeRegion,
  BadgeSource,
  ChromeActions,
  ChromeCanvas,
  ChromeHoverState,
  ChromeInstanceState,
  ChromeMetadata,
  HitRegion,
} from './CubeChromeContracts.js';

interface RendererOptions {
  getActions: () => ChromeActions;
  getHoverState: () => ChromeHoverState;
  resolveSource: ((metadata: ChromeMetadata) => BadgeSource | null) | null;
  requestRedraw: () => void;
  painter: CubeChromePainter;
}

/** Render managed Cube headers and publish their hit regions. */
export class CubeChromeRenderer {
  private readonly animationState = new CubeChromeAnimationState();
  private readonly resolveSource: ((metadata: ChromeMetadata) => BadgeSource | null) | null;
  private readonly painter: CubeChromePainter;
  private readonly contentPainter: CubeChromeHeaderContentPainter;
  readonly hitRegions: HitRegion[] = [];
  readonly badgeRegions: BadgeRegion[] = [];
  lastTransform: DOMMatrix | null = null;

  constructor(private readonly options: RendererOptions) {
    this.resolveSource = options.resolveSource;
    this.painter = options.painter;
    this.contentPainter = new CubeChromeHeaderContentPainter(this.painter);
  }
  private get actions(): ChromeActions {
    return this.options.getActions();
  }
  clear(): void {
    this.hitRegions.length = 0;
    this.badgeRegions.length = 0;
    this.animationState.clear();
  }

  render(ctx: CanvasRenderingContext2D, canvasInstance: ChromeCanvas): void {
    if (!ctx || !canvasInstance) {
      return;
    }
    const graph = canvasInstance.graph;
    if (!graph) {
      return;
    }
    this.hitRegions.length = 0;
    this.badgeRegions.length = 0;
    ctx.save();
    this.lastTransform = typeof ctx.getTransform === 'function' ? ctx.getTransform() : null;
    const groups = getGraphGroups(graph);
    if (!groups.length) {
      ctx.restore();
      return;
    }
    ctx.textBaseline = 'middle';
    for (const group of groups) {
      const metadata = getGroupSugarcubes(group) as ChromeMetadata | null;
      if (!metadata?.managed || !metadata.instance_id) {
        continue;
      }
      const bounds = readGroupBounds(group);
      if (!bounds) {
        continue;
      }
      this.renderHeader(ctx, bounds, metadata, group);
    }
    ctx.restore();
  }

  renderHeader(
    ctx: CanvasRenderingContext2D,
    bounds: number[],
    metadata: ChromeMetadata,
    group: ComfyGroup = {},
  ): void {
    const now = Date.now();
    const [x = 0, y = 0, w = 0] = bounds;
    const fontSize = Math.max(14, Number(metadata?.font_size) || 18);
    const inset = 8;
    const gap = 6;
    const paddingX = 8;
    const paddingY = 4;
    const titleColor = resolveGroupTitleColor(group);
    const swapLeftColor = triadicColor(titleColor, -120);
    const swapRightColor = triadicColor(titleColor, 120);
    const items: ChromeItem[] = [];
    const instanceId = metadata?.instance_id || '';
    const state = this.ensureInstanceState(instanceId);

    const isDirty = Boolean(metadata?.has_saveable_changes);
    const showSaved = Boolean(state.savedAt && now - state.savedAt < 350);
    const chromeContext = {
      isDirty,
      showSaved,
      canSwap: hasSwapEligibility(metadata),
    };
    for (const entry of CHROME_BUTTONS) {
      if (entry.requires && !this.actions[entry.requires]) {
        continue;
      }
      if (entry.key === 'swap-left') {
        const allowed = this.actions.canSwap
          ? this.actions.canSwap(metadata, 'left')
          : chromeContext.canSwap;
        if (!allowed) {
          continue;
        }
      }
      if (entry.key === 'swap-right') {
        const allowed = this.actions.canSwap
          ? this.actions.canSwap(metadata, 'right')
          : chromeContext.canSwap;
        if (!allowed) {
          continue;
        }
      }
      if (typeof entry.visible === 'function' && !entry.visible(chromeContext)) {
        continue;
      }
      const label = typeof entry.buildLabel === 'function' ? entry.buildLabel(chromeContext) : null;
      const extra = typeof entry.buildExtra === 'function' ? entry.buildExtra(chromeContext) : null;
      let itemColor = entry.color;
      if (entry.key === 'swap-left') {
        itemColor = swapLeftColor;
      }
      if (entry.key === 'swap-right') {
        itemColor = swapRightColor;
      }
      items.push({
        key: entry.key,
        icon: entry.icon,
        color: entry.key === 'menu' ? titleColor : itemColor,
        label: label ?? entry.label,
        style: entry.style,
        tooltip: entry.tooltip,
        ...(extra || {}),
      });
    }

    ctx.font = `${fontSize}px sans-serif`;
    const maxWidth = w - inset * 2;
    const headerHeight = Number(metadata?.bounds?.header?.height) || 32;
    const availableHeight = Math.max(0, headerHeight - paddingY * 2);
    const baseFont = Math.max(12, Number(metadata?.font_size) || 18);
    const nameSize = clampNumber(baseFont, 12, Math.max(12, Math.floor(availableHeight * 0.6)));
    const authorSize = clampNumber(
      nameSize - 2,
      10,
      Math.max(10, Math.floor(availableHeight * 0.4)),
    );
    const lineGap = 2;
    const fontFamily = resolveGroupFontFamily();
    const titlePadding = resolveGroupTitlePadding();
    const groupTitleSize = Number(group?.font_size) || fontSize;
    const resolvedDisplayName = resolveCubeDisplayName({
      metadata,
      group,
      fallback: 'SugarCube',
    });
    const currentInstanceTitle = resolveInstanceDisplayName({
      metadata,
      group,
      fallback: resolvedDisplayName,
    });
    let instanceTitleWidth = 0;
    if (currentInstanceTitle) {
      ctx.save();
      ctx.font = `${groupTitleSize}px ${fontFamily}`;
      instanceTitleWidth = ctx.measureText(currentInstanceTitle).width;
      ctx.restore();
    }
    const versionText = formatCubeVersionText(metadata);
    const displayName = versionText ? `${resolvedDisplayName} ${versionText}` : resolvedDisplayName;
    const fallbackSource =
      typeof this.resolveSource === 'function' ? this.resolveSource(metadata) : null;
    const sourceLine = formatCubeSourceText(metadata, fallbackSource);
    const badgeSizes = { name: nameSize, author: authorSize };
    const iconSize = clampNumber(Math.floor(availableHeight), 18, 32);
    const iconGap = 7;
    const titlebarLeftWidth = iconSize + iconGap + instanceTitleWidth + titlePadding + 12;
    const pinnedPillKey = 'menu';
    const pillLayout = computePillLayout(ctx, items, maxWidth, fontSize, paddingX, gap, {
      pinnedKey: pinnedPillKey,
    });
    const pillEntries = pillLayout.entries;
    let pillsTotalWidth = pillLayout.totalWidth;
    const computePillStart = () =>
      pillEntries.length ? x + w - inset - (pillsTotalWidth ? pillsTotalWidth : 0) : x + w - inset;
    const computeBadgeSlot = () =>
      computeCenteredBadgeSlot({
        groupX: x,
        groupWidth: w,
        inset,
        titlebarLeftWidth,
        pillStart: computePillStart(),
        gap,
      });
    let centeredBadgeSlot = computeBadgeSlot();
    let badgeMaxWidth = Math.min(CHROME_BADGE_MAX_WIDTH, centeredBadgeSlot.width);
    let stackedBadgeLayout = buildStackedBadgeLayout(
      ctx,
      displayName,
      sourceLine,
      badgeMaxWidth,
      paddingX,
      badgeSizes,
      fontFamily,
    );
    while (
      stackedBadgeLayout.visible &&
      centeredBadgeSlot.width < CHROME_BADGE_MIN_WIDTH &&
      pillEntries.length
    ) {
      if (!removeUnpinnedPillEntry(pillEntries, pinnedPillKey)) {
        break;
      }
      pillsTotalWidth = sumPillWidths(pillEntries, gap);
      centeredBadgeSlot = computeBadgeSlot();
      badgeMaxWidth = Math.min(CHROME_BADGE_MAX_WIDTH, centeredBadgeSlot.width);
      stackedBadgeLayout = buildStackedBadgeLayout(
        ctx,
        displayName,
        sourceLine,
        badgeMaxWidth,
        paddingX,
        badgeSizes,
        fontFamily,
      );
      if (!centeredBadgeSlot.width) {
        break;
      }
    }
    if (stackedBadgeLayout.visible && centeredBadgeSlot.width < CHROME_BADGE_MIN_WIDTH) {
      stackedBadgeLayout = {
        visible: false,
        width: 0,
        lines: [],
        truncated: false,
      };
    }
    const cursorX = x + w - inset - (pillsTotalWidth || 0);
    const centerY = y + fontSize;
    const pillHeight = fontSize + paddingY * 2;
    const badgeHeight = nameSize + authorSize + lineGap + paddingY * 2;
    const baseAlpha = Number.isFinite(ctx.globalAlpha) ? ctx.globalAlpha : 1;

    if (!pillEntries.length && !stackedBadgeLayout.visible) {
      return;
    }

    if (isDirty && !state.dirty) {
      state.dirty = true;
      state.appearAt = now;
    } else if (!isDirty && state.dirty) {
      state.dirty = false;
      state.appearAt = 0;
    }
    const isAnimating = this.isAnimating(state, now);

    if (stackedBadgeLayout.visible) {
      centeredBadgeSlot = computeBadgeSlot();
      if (stackedBadgeLayout.width > centeredBadgeSlot.width) {
        stackedBadgeLayout = {
          visible: false,
          width: 0,
          lines: [],
          truncated: false,
        };
      }
    }

    this.contentPainter.paintBadge({
      ctx,
      layout: stackedBadgeLayout,
      slot: centeredBadgeSlot,
      centerY,
      badgeHeight,
      paddingY,
      lineGap,
      nameSize,
      fontFamily,
      baseAlpha,
      group,
      instanceId,
      fontSize,
      hover: this.options.getHoverState(),
      regions: this.badgeRegions,
    });
    this.contentPainter.paintPills({
      ctx,
      entries: pillEntries,
      cursorX,
      centerY,
      pillHeight,
      gap,
      instanceId,
      now,
      fontSize,
      metadata,
      hover: this.options.getHoverState(),
      regions: this.hitRegions,
    });
    if (isAnimating) {
      this.options.requestRedraw();
    }
  }

  isAnimating(state: ChromeInstanceState | null, now: number): boolean {
    return this.animationState.isAnimating(state, now);
  }

  ensureInstanceState(instanceId: string): ChromeInstanceState {
    return this.animationState.ensure(instanceId);
  }

  isHovered(instanceId: string, key: string): boolean {
    if (!key) return false;
    const hover = this.options.getHoverState();
    if (!hover.hoveredKey || hover.hoveredKey !== key) return false;
    return !instanceId || hover.hoveredInstance === instanceId;
  }
}
