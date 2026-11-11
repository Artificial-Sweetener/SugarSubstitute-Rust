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
 * Compute immutable Cube chrome colors, text measurements, and layout.
 */

import type { ComfyGroup } from '../types/graph.js';
import type { UnknownRecord } from '../types/common.js';
import type { ChromeMetadata } from './CubeChromeContracts.js';
export { triadicColor } from './CubeChromeColor.js';

export interface ChromeContext {
  isDirty: boolean;
  showSaved: boolean;
  canSwap: boolean;
}
interface ChromeButtonDefinition extends UnknownRecord {
  key: string;
  style: string;
  tooltip: string;
  requires?: string;
  label?: string;
  icon?: string;
  labelAlign?: string;
  labelScale?: number;
  color?: string;
  visible?: (context: ChromeContext) => boolean;
  buildLabel?: (context: ChromeContext) => string | null;
  buildExtra?: (context: ChromeContext) => UnknownRecord | null;
}
export interface ChromeItem extends UnknownRecord {
  key: string;
  icon?: string | undefined;
  color?: string | undefined;
  label?: string | undefined;
  style?: string | undefined;
  tooltip?: string | undefined;
  labelAlign?: string;
  labelScale?: number;
  flavorOptions?: unknown;
}
export interface PillEntry {
  item: ChromeItem;
  pillWidth: number;
}
interface BadgeLine {
  key: string;
  text: string;
  fullText: string;
  size: number;
  width: number;
  truncated: boolean;
}
export interface BadgeLayout {
  visible: boolean;
  width: number;
  lines: BadgeLine[];
  truncated: boolean;
}
interface TextMeasure {
  text: string;
  width: number;
  truncated: boolean;
}

/** Bound the preferred definition badge width. */
export const CHROME_BADGE_MAX_WIDTH = 280;
/** Preserve a readable minimum definition badge width. */
export const CHROME_BADGE_MIN_WIDTH = 80;
/** Define ordered action pills rendered in Cube headers. */
export const CHROME_BUTTONS: readonly ChromeButtonDefinition[] = Object.freeze([
  {
    key: 'swap-left',
    style: 'action',
    tooltip: 'Swap left',
    requires: 'onSwapLeft',
    label: '⇦',
    labelAlign: 'center',
    labelScale: 1.15,
  },
  {
    key: 'swap-right',
    style: 'action',
    tooltip: 'Swap right',
    requires: 'onSwapRight',
    label: '⇨',
    labelAlign: 'center',
    labelScale: 1.15,
  },
  {
    key: 'menu',
    icon: 'cube',
    style: 'action',
    tooltip: 'Cubes',
  },
]);

function measureTextEllipsis(
  ctx: CanvasRenderingContext2D,
  text: string,
  maxWidth: number,
): TextMeasure {
  if (!text) {
    return { text: '', width: 0, truncated: false };
  }
  if (maxWidth <= 0) {
    return { text: '', width: 0, truncated: true };
  }
  const fullWidth = ctx.measureText(text).width;
  if (fullWidth <= maxWidth) {
    return { text, width: fullWidth, truncated: false };
  }
  const ellipsis = '...';
  const ellipsisWidth = ctx.measureText(ellipsis).width;
  if (ellipsisWidth >= maxWidth) {
    return { text: '', width: 0, truncated: true };
  }
  let low = 0;
  let high = text.length;
  while (low < high) {
    const mid = Math.ceil((low + high) / 2);
    const candidate = `${text.slice(0, mid)}${ellipsis}`;
    const width = ctx.measureText(candidate).width;
    if (width <= maxWidth) {
      low = mid;
    } else {
      high = mid - 1;
    }
  }
  const trimmed = `${text.slice(0, low)}${ellipsis}`;
  return { text: trimmed, width: ctx.measureText(trimmed).width, truncated: true };
}

/** Clamp a finite numeric value to an inclusive range. */
export function clampNumber(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) {
    return min;
  }
  return Math.min(max, Math.max(min, value));
}

/** Resolve the host group title color with a stable fallback. */
export function resolveGroupTitleColor(group: ComfyGroup): string {
  const color = typeof group?.color === 'string' ? group.color.trim() : '';
  if (color) {
    return color;
  }
  const bg = typeof group?.bgcolor === 'string' ? group.bgcolor.trim() : '';
  if (bg) {
    return bg;
  }
  return '#9ab4c7';
}

/** Resolve the LiteGraph group title font family. */
export function resolveGroupFontFamily(): string {
  const liteGraph = typeof globalThis !== 'undefined' ? globalThis.LiteGraph : null;
  const font = typeof liteGraph?.GROUP_FONT === 'string' ? liteGraph.GROUP_FONT.trim() : '';
  return font || 'sans-serif';
}

/** Resolve the LiteGraph group title horizontal padding. */
export function resolveGroupTitlePadding(): number {
  const liteGraph = typeof globalThis !== 'undefined' ? globalThis.LiteGraph : null;
  const padding = liteGraph?.LGraphGroup?.padding;
  if (Number.isFinite(padding)) {
    return Number(padding);
  }
  return 4;
}

/** Measure an ordered pill row including inter-pill gaps. */
export function sumPillWidths(entries: readonly PillEntry[], gap: number): number {
  if (!entries.length) {
    return 0;
  }
  return entries.reduce((sum, entry) => sum + entry.pillWidth, 0) + gap * (entries.length - 1);
}
/** Fit the definition and source labels into a stacked badge. */
export function buildStackedBadgeLayout(
  ctx: CanvasRenderingContext2D,
  nameText: unknown,
  authorText: unknown,
  maxWidth: number,
  paddingX: number,
  sizes: { name: number; author: number },
  fontFamily: string,
): BadgeLayout {
  const trimmedName = typeof nameText === 'string' ? nameText.trim() : '';
  const trimmedAuthor = typeof authorText === 'string' ? authorText.trim() : '';
  if (!trimmedName) {
    return {
      visible: false,
      width: 0,
      lines: [],
      truncated: false,
    };
  }
  const textMaxWidth = Math.max(0, maxWidth - paddingX * 2);
  const lines = [
    { key: 'name', text: trimmedName, size: sizes.name },
    { key: 'author', text: trimmedAuthor, size: sizes.author },
  ];
  const family = fontFamily || 'sans-serif';
  const measured = lines.map((line) => {
    ctx.font = `${line.size}px ${family}`;
    const measurement = measureTextEllipsis(ctx, line.text, textMaxWidth);
    return {
      key: line.key,
      size: line.size,
      text: measurement.text,
      width: measurement.width,
      truncated: measurement.truncated,
      fullText: line.text,
    };
  });
  const maxLineWidth = measured.reduce((max, entry) => Math.max(max, entry.width), 0);
  const width = maxLineWidth + paddingX * 2;
  return {
    visible: width > 0,
    width,
    lines: measured,
    truncated: measured.some((entry) => entry.truncated),
  };
}

/**
 * Resolve the centered badge slot without overlapping titlebar chrome.
 */
/** Compute the centered header span left between title and action chrome. */
export function computeCenteredBadgeSlot({
  groupX,
  groupWidth,
  inset,
  titlebarLeftWidth,
  pillStart,
  gap,
}: {
  groupX: number;
  groupWidth: number;
  inset: number;
  titlebarLeftWidth: number;
  pillStart: number;
  gap: number;
}): { left: number; right: number; center: number; width: number } {
  const left = Math.min(groupX + inset + titlebarLeftWidth, groupX + groupWidth - inset);
  const right = Math.max(left, pillStart - gap);
  return {
    left,
    right,
    center: groupX + groupWidth / 2,
    width: Math.max(0, right - left),
  };
}

/** Fit action pills into the available header width. */
export function computePillLayout(
  ctx: CanvasRenderingContext2D,
  items: ChromeItem[],
  maxWidth: number,
  fontSize: number,
  paddingX: number,
  gap: number,
  options: { pinnedKey?: string } = {},
): { entries: PillEntry[]; totalWidth: number } {
  const pinnedKey = typeof options?.pinnedKey === 'string' ? options.pinnedKey : '';
  const entries = items.map((item) => {
    const iconSize = item.icon ? Math.max(12, fontSize - 2) : 0;
    const textWidth = item.label ? ctx.measureText(item.label).width : 0;
    const contentWidth = item.icon ? iconSize : textWidth;
    const pillWidth = contentWidth + paddingX * 2;
    return { item, pillWidth };
  });
  const pinIndex = pinnedKey ? entries.findIndex((entry) => entry.item?.key === pinnedKey) : -1;
  while (entries.length) {
    const total = sumPillWidths(entries, gap);
    if (total <= maxWidth) {
      break;
    }
    if (pinIndex >= 0) {
      const candidateIndex = entries.findIndex((entry) => entry.item?.key !== pinnedKey);
      if (candidateIndex === -1) {
        break;
      }
      entries.splice(candidateIndex, 1);
      continue;
    }
    entries.pop();
  }
  return { entries, totalWidth: sumPillWidths(entries, gap) };
}

/**
 * Drop a lower-priority action pill while preserving pinned chrome controls.
 */
/** Remove one lower-priority pill while retaining the pinned control. */
export function removeUnpinnedPillEntry(entries: PillEntry[], pinnedKey: string): boolean {
  const index = entries.findIndex((entry) => entry.item?.key !== pinnedKey);
  if (index < 0) {
    return false;
  }
  entries.splice(index, 1);
  return true;
}

/** Report whether metadata exposes both input and output markers. */
export function hasSwapEligibility(metadata: ChromeMetadata): boolean {
  const markers = metadata?.markers;
  const inputs = Array.isArray(markers?.inputs) ? markers.inputs : [];
  const outputs = Array.isArray(markers?.outputs) ? markers.outputs : [];
  return inputs.length > 0 && outputs.length > 0;
}
