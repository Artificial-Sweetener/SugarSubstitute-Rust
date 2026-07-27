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
/** Own deterministic responsive masonry geometry for Cube surface cards. */

export interface CubeMasonryCard {
  id: string;
  height: number;
  columnSpan?: number;
  sourceX?: number;
  sourceY?: number;
  sourceWidth?: number;
}

export interface CubeMasonryOptions {
  availableWidth: number;
  minimumColumnWidth: number;
  gap: number;
}

export interface CubeMasonryPlacement extends CubeMasonryCard {
  column: number;
  x: number;
  y: number;
  width: number;
}

export interface CubeMasonryResult {
  columnCount: number;
  columnWidth: number;
  height: number;
  placements: CubeMasonryPlacement[];
}

/** Compute a finite shortest-column masonry layout for the available width. */
export function computeCubeMasonry(
  cards: readonly CubeMasonryCard[],
  options: CubeMasonryOptions,
): CubeMasonryResult {
  const gap = finiteNonNegative(options.gap);
  const availableWidth = Math.max(1, finiteNonNegative(options.availableWidth));
  const minimumColumnWidth = Math.max(1, finiteNonNegative(options.minimumColumnWidth));
  const widthColumnCapacity = Math.max(
    1,
    Math.floor((availableWidth + gap) / (minimumColumnWidth + gap)),
  );
  const columnCount = Math.max(1, Math.min(widthColumnCapacity, Math.max(1, cards.length)));
  const columnWidth = (availableWidth - gap * (columnCount - 1)) / columnCount;
  const spatialPlacements = computeSpatialPlacements(
    cards,
    columnCount,
    columnWidth,
    minimumColumnWidth,
    gap,
  );
  if (spatialPlacements) return spatialPlacements;
  const columnHeights = Array.from({ length: columnCount }, () => 0);
  const placements: CubeMasonryPlacement[] = [];

  for (const card of cards) {
    const height = finiteNonNegative(card.height);
    const columnSpan = normalizedColumnSpan(card.columnSpan, columnCount);
    const column = indexOfLowestSpanWindow(columnHeights, columnSpan);
    const y = maximum(columnHeights.slice(column, column + columnSpan));
    placements.push({
      id: card.id,
      column,
      x: column * (columnWidth + gap),
      y,
      width: columnWidth * columnSpan + gap * (columnSpan - 1),
      height,
    });
    for (let index = column; index < column + columnSpan; index += 1) {
      columnHeights[index] = y + height + gap;
    }
  }

  const occupiedHeights = columnHeights.map((height) => Math.max(0, height - gap));
  return {
    columnCount,
    columnWidth,
    height: Math.max(0, ...occupiedHeights),
    placements,
  };
}

interface SpatialCard {
  card: CubeMasonryCard;
  index: number;
  centerX: number;
  sourceY: number;
}

/** Preserve saved editor columns while splitting or merging them responsively. */
function computeSpatialPlacements(
  cards: readonly CubeMasonryCard[],
  columnCount: number,
  columnWidth: number,
  minimumColumnWidth: number,
  gap: number,
): CubeMasonryResult | null {
  // Saved editor columns describe independent single-column cards; a spanning prompt owns a
  // contiguous responsive window instead, so it must use the shared span-aware placement path.
  if (cards.some((card) => normalizedColumnSpan(card.columnSpan, columnCount) > 1)) return null;
  const spatialCards = cards.map(toSpatialCard);
  if (spatialCards.some((card) => card === null)) return null;
  const ordered = (spatialCards as SpatialCard[]).sort(
    (left, right) =>
      left.centerX - right.centerX || left.sourceY - right.sourceY || left.index - right.index,
  );
  let columns = clusterSpatialColumns(ordered, Math.max(24, minimumColumnWidth / 2));
  while (columns.length > columnCount) columns = mergeShortestAdjacentColumns(columns, gap);
  while (columns.length < columnCount) {
    const splitIndex = indexOfTallestSplittableColumn(columns, gap);
    if (splitIndex < 0) break;
    const source = columns[splitIndex];
    if (!source) break;
    const [left, right] = splitBalancedColumn(source, gap);
    columns.splice(splitIndex, 1, left, right);
  }

  const placements = new Array<CubeMasonryPlacement>(cards.length);
  let layoutHeight = 0;
  for (const [column, groupedCards] of columns.entries()) {
    let y = 0;
    for (const spatialCard of groupedCards) {
      const height = finiteNonNegative(spatialCard.card.height);
      placements[spatialCard.index] = {
        ...spatialCard.card,
        column,
        x: column * (columnWidth + gap),
        y,
        width: columnWidth,
        height,
      };
      y += height + gap;
    }
    layoutHeight = Math.max(layoutHeight, Math.max(0, y - gap));
  }
  return {
    columnCount: columns.length,
    columnWidth,
    height: layoutHeight,
    placements,
  };
}

/** Narrow one card's persisted editor geometry. */
function toSpatialCard(card: CubeMasonryCard, index: number): SpatialCard | null {
  const sourceX = Number(card.sourceX);
  const sourceY = Number(card.sourceY);
  const sourceWidth = Number(card.sourceWidth ?? 0);
  if (!Number.isFinite(sourceX) || !Number.isFinite(sourceY)) return null;
  return {
    card,
    index,
    centerX: sourceX + (Number.isFinite(sourceWidth) ? Math.max(0, sourceWidth) / 2 : 0),
    sourceY,
  };
}

/** Group horizontally adjacent editor nodes into their saved visual columns. */
function clusterSpatialColumns(cards: readonly SpatialCard[], tolerance: number): SpatialCard[][] {
  const columns: SpatialCard[][] = [];
  let activeCenter = Number.NEGATIVE_INFINITY;
  for (const card of cards) {
    const active = columns.at(-1);
    if (!active || card.centerX - activeCenter > tolerance) {
      columns.push([card]);
      activeCenter = card.centerX;
      continue;
    }
    active.push(card);
    activeCenter = active.reduce((sum, item) => sum + item.centerX, 0) / active.length;
  }
  for (const column of columns) {
    column.sort(
      (left, right) =>
        left.sourceY - right.sourceY || left.centerX - right.centerX || left.index - right.index,
    );
  }
  return columns;
}

/** Merge the least expensive adjacent editor columns while preserving horizontal order. */
function mergeShortestAdjacentColumns(
  columns: readonly SpatialCard[][],
  gap: number,
): SpatialCard[][] {
  let mergeIndex = 0;
  let minimumHeight = Number.POSITIVE_INFINITY;
  for (let index = 0; index < columns.length - 1; index += 1) {
    const height =
      columnHeight(columns[index] ?? [], gap) + columnHeight(columns[index + 1] ?? [], gap);
    if (height < minimumHeight) {
      minimumHeight = height;
      mergeIndex = index;
    }
  }
  return columns.flatMap((column, index) =>
    index === mergeIndex
      ? [[...column, ...(columns[index + 1] ?? [])]]
      : index === mergeIndex + 1
        ? []
        : [column],
  );
}

/** Find the tallest column that can supply one additional responsive column. */
function indexOfTallestSplittableColumn(columns: readonly SpatialCard[][], gap: number): number {
  let result = -1;
  let maximumHeight = Number.NEGATIVE_INFINITY;
  for (const [index, column] of columns.entries()) {
    const height = column.length > 1 ? columnHeight(column, gap) : Number.NEGATIVE_INFINITY;
    if (height > maximumHeight) {
      maximumHeight = height;
      result = index;
    }
  }
  return result;
}

/** Split one ordered column at the most balanced contiguous boundary. */
function splitBalancedColumn(
  column: readonly SpatialCard[],
  gap: number,
): [SpatialCard[], SpatialCard[]] {
  let splitIndex = 1;
  let minimumDifference = Number.POSITIVE_INFINITY;
  for (let index = 1; index < column.length; index += 1) {
    const difference = Math.abs(
      columnHeight(column.slice(0, index), gap) - columnHeight(column.slice(index), gap),
    );
    if (difference < minimumDifference) {
      minimumDifference = difference;
      splitIndex = index;
    }
  }
  return [column.slice(0, splitIndex), column.slice(splitIndex)];
}

/** Measure one finite stacked column. */
function columnHeight(column: readonly SpatialCard[], gap: number): number {
  return column.reduce(
    (height, card, index) => height + finiteNonNegative(card.card.height) + (index > 0 ? gap : 0),
    0,
  );
}

/** Reconcile persisted card order with the nodes currently owned by the Cube graph. */
export function orderCubeSurfaceCards(
  persistedOrder: readonly string[],
  availableNodeIds: readonly string[],
): string[] {
  const available = new Set(availableNodeIds);
  const ordered: string[] = [];
  const seen = new Set<string>();

  for (const nodeId of [...persistedOrder, ...availableNodeIds]) {
    if (!available.has(nodeId) || seen.has(nodeId)) continue;
    seen.add(nodeId);
    ordered.push(nodeId);
  }
  return ordered;
}

/** Return a safe finite non-negative geometry value. */
function finiteNonNegative(value: number): number {
  return Number.isFinite(value) ? Math.max(0, value) : 0;
}

/** Clamp one requested masonry span to the currently available responsive columns. */
function normalizedColumnSpan(value: number | undefined, columnCount: number): number {
  const candidate = Number.isFinite(value) ? Math.floor(value ?? 1) : 1;
  return Math.max(1, Math.min(columnCount, candidate));
}

/** Find the contiguous span whose tallest occupied column is lowest. */
function indexOfLowestSpanWindow(values: readonly number[], span: number): number {
  let winner = 0;
  let winningHeight = Number.POSITIVE_INFINITY;
  for (let column = 0; column <= values.length - span; column += 1) {
    const height = maximum(values.slice(column, column + span));
    if (height < winningHeight) {
      winner = column;
      winningHeight = height;
    }
  }
  return winner;
}

/** Return a finite greatest value for a non-empty collection of column heights. */
function maximum(values: readonly number[]): number {
  return Math.max(0, ...values);
}
