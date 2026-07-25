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
/** Compute renderer-independent Nodes 1.0 Cube surface geometry. */

import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import { CUBE_CANVAS_ACTIVATION_SIZE } from './CubeCanvasActivationControl.js';
import { CUBE_RESIZE_EDGES, type CubeResizeEdge } from '../cube/geometry/CubeResizeGeometry.js';
import type { Vec2 } from '../types/common.js';
import type { ComfyNode } from '../types/graph.js';
import {
  resolveCubeFaceCardPresentation,
  type CubeFaceCardMenuEntry,
} from './CubeFaceCardPolicy.js';
import { measureCubeFaceNodeBodyHeight } from './CubeFaceNodeMeasurement.js';
import { computeCubeMasonry, orderCubeSurfaceCards } from './CubeMasonryLayout.js';
import { cubeMinimumSize, resolveCubeSurfaceMinimumHeight } from './CubeSurfaceMinimumHeight.js';
import { resolveCubeSurfaceCardSpacing } from './CubeSurfaceSpacing.js';
import type { CubeSurfaceState } from './CubeSurfaceState.js';
import { resolveCubePreviewTitleAnchors } from './CubePreviewSections.js';
import type { CubeFaceTitlebarActionKey } from './CubeFaceChromeActions.js';
import { layoutCubeCanvasChrome } from './CubeCanvasChromeLayout.js';

export interface CubeCanvasRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface CubeCanvasCard {
  node: ComfyNode;
  id: string;
  label: string;
  enabled: boolean;
  showActivationControl: boolean;
  bodyHeight: number;
  rect: CubeCanvasRect;
  activationAction: CubeCanvasRect | null;
}

export interface CubeCanvasPort {
  index: number;
  name: string;
  type: string;
  x: number;
  y: number;
  slot: unknown;
}

export interface CubeCanvasResizeHandle {
  edge: CubeResizeEdge;
  rect: CubeCanvasRect;
}

export interface CubeCanvasLayout {
  frame: CubeCanvasRect;
  header: CubeCanvasRect;
  content: CubeCanvasRect;
  masonry: CubeCanvasRect;
  preview: CubeCanvasRect | null;
  editAction: CubeCanvasRect;
  cardMenuAction: CubeCanvasRect;
  chromeActions: Readonly<Partial<Record<CubeFaceTitlebarActionKey, CubeCanvasRect>>>;
  cardMenuEntries: CubeFaceCardMenuEntry[];
  resizeHandles: CubeCanvasResizeHandle[];
  cards: CubeCanvasCard[];
  inputs: CubeCanvasPort[];
  outputs: CubeCanvasPort[];
  minimumSize: Vec2;
}

const FRAME_PADDING = 12;
const HEADER_HEIGHT = 42;
const CONTENT_GAP = 12;
const RESIZE_HANDLE_SIZE = 18;
const RESIZE_EDGE_THICKNESS = 10;

/** Lay out one finite canvas Cube using the same persisted masonry policy as Nodes 2.0. */
export function computeCubeCanvasLayout(
  node: CubeNode,
  state: CubeSurfaceState,
  titleHeight: number,
  titlebarActionKeys: readonly CubeFaceTitlebarActionKey[] = [],
): CubeCanvasLayout {
  const frame = rect(
    Number(node.pos[0]),
    Number(node.pos[1]) - titleHeight,
    Number(node.size[0]),
    Number(node.size[1]) + titleHeight,
  );
  const header = rect(frame.x, frame.y, frame.width, HEADER_HEIGHT);
  const spacing = resolveCubeSurfaceCardSpacing(state);
  const content = rect(
    frame.x + FRAME_PADDING,
    frame.y + HEADER_HEIGHT + spacing.headerInset,
    Math.max(1, frame.width - FRAME_PADDING * 2),
    Math.max(1, frame.height - HEADER_HEIGHT - spacing.headerInset - spacing.footerInset),
  );
  const minimumMasonryWidth = Math.min(content.width, Math.max(1, state.minimumColumnWidth));
  const previewWidth = state.preview.visible
    ? Math.min(state.preview.width, Math.max(0, content.width - CONTENT_GAP - minimumMasonryWidth))
    : 0;
  const masonryWidth = Math.max(
    1,
    content.width - (previewWidth > 0 ? previewWidth + CONTENT_GAP : 0),
  );
  const masonry = rect(content.x, content.y, masonryWidth, content.height);
  const preview =
    previewWidth > 0
      ? rect(content.x + masonryWidth + CONTENT_GAP, content.y, previewWidth, content.height)
      : null;
  const orderedNodes = orderNodes(node.subgraph._nodes, state.nodeOrder);
  const presentation = resolveCubeFaceCardPresentation(orderedNodes, state);
  const chrome = layoutCubeCanvasChrome(header, {
    showCardMenu: presentation.menuEntries.length > 0,
    titlebarActionKeys,
  });
  const measuredCards = presentation.cards
    .filter((card) => card.visible)
    .map((card) => ({
      ...card,
      bodyHeight: measureCubeFaceNodeBodyHeight(card.node),
    }));
  const masonryLayout = computeCubeMasonry(
    measuredCards.map((card) => ({
      id: card.id,
      height: card.bodyHeight + titleHeight,
      sourceX: Number(card.node.pos?.[0]),
      sourceY: Number(card.node.pos?.[1]),
      sourceWidth: Number(card.node.size?.[0]),
    })),
    {
      availableWidth: masonry.width,
      minimumColumnWidth: state.minimumColumnWidth,
      gap: spacing.gap,
    },
  );
  const cards = masonryLayout.placements.map((placement, index) => {
    const card = measuredCards[index];
    if (!card) throw new RangeError('Cube masonry placement has no corresponding node.');
    const cardRect = rect(
      masonry.x + placement.x,
      masonry.y + placement.y,
      placement.width,
      placement.height,
    );
    return {
      node: card.node,
      id: card.id,
      label: card.label,
      enabled: card.enabled,
      showActivationControl: card.showActivationControl,
      bodyHeight: card.bodyHeight,
      rect: cardRect,
      activationAction: card.showActivationControl
        ? rect(
            cardRect.x + cardRect.width - CUBE_CANVAS_ACTIVATION_SIZE.width - 6,
            cardRect.y + (titleHeight - CUBE_CANVAS_ACTIVATION_SIZE.height) / 2,
            CUBE_CANVAS_ACTIVATION_SIZE.width,
            CUBE_CANVAS_ACTIVATION_SIZE.height,
          )
        : null,
    };
  });
  const minimumSize = cubeMinimumSize(
    resolveCubeSurfaceMinimumHeight({
      contentHeight: masonryLayout.height,
      headerInset: spacing.headerInset,
      footerInset: spacing.footerInset,
      faceHeaderHeight: HEADER_HEIGHT,
      nativeTitleHeight: titleHeight,
    }),
  );
  return {
    frame,
    header,
    content,
    masonry,
    preview,
    editAction: chrome.editAction,
    cardMenuAction: chrome.cardMenuAction,
    chromeActions: chrome.chromeActions,
    cardMenuEntries: presentation.menuEntries,
    resizeHandles: layoutResizeHandles(frame),
    cards,
    inputs: layoutPorts(node.subgraph.inputs, frame, 'input'),
    outputs: layoutOutputPorts(node.outputs, frame, preview),
    minimumSize,
  };
}

/** Place four edge and four corner hit targets around one canvas frame. */
function layoutResizeHandles(frame: CubeCanvasRect): CubeCanvasResizeHandle[] {
  const byEdge: Record<CubeResizeEdge, CubeCanvasRect> = {
    n: rect(
      frame.x + RESIZE_HANDLE_SIZE,
      frame.y,
      frame.width - RESIZE_HANDLE_SIZE * 2,
      RESIZE_EDGE_THICKNESS,
    ),
    ne: rect(
      frame.x + frame.width - RESIZE_HANDLE_SIZE,
      frame.y,
      RESIZE_HANDLE_SIZE,
      RESIZE_HANDLE_SIZE,
    ),
    e: rect(
      frame.x + frame.width - RESIZE_EDGE_THICKNESS,
      frame.y + RESIZE_HANDLE_SIZE,
      RESIZE_EDGE_THICKNESS,
      frame.height - RESIZE_HANDLE_SIZE * 2,
    ),
    se: rect(
      frame.x + frame.width - RESIZE_HANDLE_SIZE,
      frame.y + frame.height - RESIZE_HANDLE_SIZE,
      RESIZE_HANDLE_SIZE,
      RESIZE_HANDLE_SIZE,
    ),
    s: rect(
      frame.x + RESIZE_HANDLE_SIZE,
      frame.y + frame.height - RESIZE_EDGE_THICKNESS,
      frame.width - RESIZE_HANDLE_SIZE * 2,
      RESIZE_EDGE_THICKNESS,
    ),
    sw: rect(
      frame.x,
      frame.y + frame.height - RESIZE_HANDLE_SIZE,
      RESIZE_HANDLE_SIZE,
      RESIZE_HANDLE_SIZE,
    ),
    w: rect(
      frame.x,
      frame.y + RESIZE_HANDLE_SIZE,
      RESIZE_EDGE_THICKNESS,
      frame.height - RESIZE_HANDLE_SIZE * 2,
    ),
    nw: rect(frame.x, frame.y, RESIZE_HANDLE_SIZE, RESIZE_HANDLE_SIZE),
  };
  return CUBE_RESIZE_EDGES.map((edge) => ({ edge, rect: byEdge[edge] }));
}

/** Return whether one graph-space point lies inside a finite rectangle. */
export function containsCubeCanvasPoint(
  target: CubeCanvasRect,
  point: readonly [number, number],
): boolean {
  return (
    point[0] >= target.x &&
    point[0] <= target.x + target.width &&
    point[1] >= target.y &&
    point[1] <= target.y + target.height
  );
}

/** Reconcile real internal node objects with persisted card order. */
function orderNodes(nodes: readonly ComfyNode[], persistedOrder: readonly string[]): ComfyNode[] {
  const byId = new Map(nodes.map((node, index) => [String(node.id ?? index), node]));
  return orderCubeSurfaceCards(persistedOrder, [...byId.keys()])
    .map((id) => byId.get(id))
    .filter((node): node is ComfyNode => node !== undefined);
}

/** Distribute graph-owned boundary slots down the outer Cube edge. */
function layoutPorts(
  slots: readonly unknown[],
  frame: CubeCanvasRect,
  kind: 'input' | 'output',
): CubeCanvasPort[] {
  return slots.map((slot, index) => {
    const record = isPortRecord(slot) ? slot : {};
    return {
      index,
      name: readString(record.name) || `${kind} ${String(index + 1)}`,
      type: readString(record.type) || '*',
      x: kind === 'input' ? frame.x : frame.x + frame.width,
      y:
        frame.y +
        HEADER_HEIGHT +
        ((index + 1) * (frame.height - HEADER_HEIGHT)) / (slots.length + 1),
      slot,
    };
  });
}

/** Place graph-owned outputs across from their ordered preview-section titles. */
function layoutOutputPorts(
  slots: readonly unknown[],
  frame: CubeCanvasRect,
  preview: CubeCanvasRect | null,
): CubeCanvasPort[] {
  const anchors = preview
    ? resolveCubePreviewTitleAnchors(preview, slots.length)
    : layoutPorts(slots, frame, 'output').map((port) => port.y);
  return slots.map((slot, index) => {
    const record = isPortRecord(slot) ? slot : {};
    return {
      index,
      name: readString(record.name) || `output ${String(index + 1)}`,
      type: readString(record.type) || '*',
      x: frame.x + frame.width,
      y: anchors[index] ?? frame.y + HEADER_HEIGHT,
      slot,
    };
  });
}

/** Build a finite rectangle without allowing invalid host geometry inward. */
function rect(x: number, y: number, width: number, height: number): CubeCanvasRect {
  return {
    x: Number.isFinite(x) ? x : 0,
    y: Number.isFinite(y) ? y : 0,
    width: Number.isFinite(width) ? Math.max(1, width) : 1,
    height: Number.isFinite(height) ? Math.max(1, height) : 1,
  };
}

/** Narrow one native slot enough to read its visible metadata. */
function isPortRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** Read a trimmed native slot value. */
function readString(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}
