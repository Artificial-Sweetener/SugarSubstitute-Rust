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
/** Apply responsive Nodes 2.0 masonry and preview-rail DOM geometry. */

import type { CubeFaceCardDecision } from './CubeFaceCardPolicy.js';
import { computeCubeMasonry } from './CubeMasonryLayout.js';
import { resolveCubeSurfaceMinimumHeight } from './CubeSurfaceMinimumHeight.js';
import { resolveCubeSurfaceCardSpacing } from './CubeSurfaceSpacing.js';
import type { CubeSurfaceState } from './CubeSurfaceState.js';
import { CUBE_DOM_SURFACE_SECTION_GAP } from './CubeSurfaceGeometry.js';
import {
  clampCubePreviewWidth,
  resolveCubePreviewWidthRange,
  type CubePreviewWidthRange,
} from './CubePreviewResizeGeometry.js';

const DEFAULT_CARD_HEIGHT = 120;
const MINIMUM_SIDE_PREVIEW_WIDTH = 240;
const STACKED_PREVIEW_MINIMUM_HEIGHT = 160;

export interface CubeSurfaceDomLayoutOptions {
  width: number;
  state: CubeSurfaceState;
  cards: readonly CubeFaceCardDecision[];
  cells: readonly HTMLElement[];
  content: HTMLElement;
  masonry: HTMLElement;
  previewDivider: HTMLButtonElement;
  previewRail: HTMLElement;
}

export interface CubeSurfaceDomLayoutResult {
  minimumHeight: number | null;
  previewWidth: number;
  previewWidthRange: CubePreviewWidthRange;
  previewResizable: boolean;
}

/** Measure native cards and apply finite responsive geometry to their containers. */
export function layoutCubeSurfaceDom(
  options: CubeSurfaceDomLayoutOptions,
): CubeSurfaceDomLayoutResult {
  const safeWidth = Number.isFinite(options.width) ? Math.max(1, options.width) : 1;
  const spacing = resolveCubeSurfaceCardSpacing(options.state);
  const minimumMasonryWidth = Math.min(safeWidth, Math.max(1, options.state.minimumColumnWidth));
  const previewEnabled = options.state.preview.visible;
  const previewWidthRange = resolveCubePreviewWidthRange(
    safeWidth,
    minimumMasonryWidth,
    CUBE_DOM_SURFACE_SECTION_GAP,
  );
  const canShowPreviewRail =
    previewEnabled &&
    safeWidth >= minimumMasonryWidth + MINIMUM_SIDE_PREVIEW_WIDTH + CUBE_DOM_SURFACE_SECTION_GAP;
  const previewWidth = canShowPreviewRail
    ? clampCubePreviewWidth(options.state.preview.width, previewWidthRange)
    : 0;
  const masonryWidth = Math.max(
    1,
    safeWidth - (canShowPreviewRail ? previewWidth + CUBE_DOM_SURFACE_SECTION_GAP : 0),
  );
  const stackPreview = previewEnabled && !canShowPreviewRail;
  const layout = computeCubeMasonry(
    options.cards.map((card, index) => ({
      id: card.id,
      height: readCardHeight(options.cells[index], card),
      ...(card.columnSpan === undefined ? {} : { columnSpan: card.columnSpan }),
    })),
    {
      availableWidth: masonryWidth,
      minimumColumnWidth: options.state.minimumColumnWidth,
      gap: spacing.gap,
    },
  );

  options.masonry.dataset.columns = String(layout.columnCount);
  options.masonry.style.width = `${masonryWidth}px`;
  options.masonry.style.height = `${layout.height}px`;
  options.content.style.setProperty(
    '--sugarcubes-cube-masonry-header-inset',
    `${spacing.headerInset}px`,
  );
  options.content.style.setProperty(
    '--sugarcubes-cube-masonry-footer-inset',
    `${spacing.footerInset}px`,
  );
  options.content.style.setProperty('--sugarcubes-cube-preview-row-gap', `${spacing.gap}px`);
  options.content.dataset.previewLayout = stackPreview
    ? 'stacked'
    : canShowPreviewRail
      ? 'rail'
      : 'hidden';
  options.content.style.flexDirection = stackPreview ? 'column' : 'row';
  options.content.style.gap = canShowPreviewRail ? '0px' : '';
  options.previewDivider.hidden = !canShowPreviewRail;
  options.previewDivider.style.flexBasis = canShowPreviewRail
    ? `${String(CUBE_DOM_SURFACE_SECTION_GAP)}px`
    : '0px';
  options.previewRail.style.width = `${stackPreview ? safeWidth : previewWidth}px`;
  options.previewRail.style.height = stackPreview
    ? `${String(STACKED_PREVIEW_MINIMUM_HEIGHT)}px`
    : 'auto';
  options.previewRail.style.minHeight = '0px';
  options.previewRail.hidden = !previewEnabled;
  const stackedPreviewHeight = stackPreview
    ? (layout.height > 0 ? CUBE_DOM_SURFACE_SECTION_GAP : 0) + STACKED_PREVIEW_MINIMUM_HEIGHT
    : 0;
  const minimumHeight = resolveCubeSurfaceMinimumHeight({
    contentHeight: layout.height + stackedPreviewHeight,
    headerInset: spacing.headerInset,
    footerInset: spacing.footerInset,
  });
  options.content.style.minHeight = `${String(minimumHeight)}px`;

  for (const [index, placement] of layout.placements.entries()) {
    const cell = options.cells[index];
    if (!cell) continue;
    cell.style.position = 'absolute';
    cell.style.left = `${placement.x}px`;
    cell.style.top = `${placement.y}px`;
    cell.style.width = `${placement.width}px`;
  }
  return {
    minimumHeight: cardsAreMeasured(options.cards, options.cells) ? minimumHeight : null,
    previewWidth,
    previewWidthRange,
    previewResizable: canShowPreviewRail,
  };
}

/** Resolve measured native-card height with a finite graph-size fallback. */
function readCardHeight(cell: HTMLElement | undefined, card: CubeFaceCardDecision): number {
  const measured = cell?.offsetHeight ?? 0;
  if (Number.isFinite(measured) && measured > 0) return measured;
  const graphHeight = Number(card.node.size?.[1]);
  return Number.isFinite(graphHeight) && graphHeight > 0 ? graphHeight : DEFAULT_CARD_HEIGHT;
}

/** Wait for native renderers to expose real card height before mutating node geometry. */
function cardsAreMeasured(
  cards: readonly CubeFaceCardDecision[],
  cells: readonly HTMLElement[],
): boolean {
  return cards.every((_, index) => {
    const measured = cells[index]?.offsetHeight ?? 0;
    return Number.isFinite(measured) && measured > 0;
  });
}
