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
/** Route only custom Nodes 1.0 face interactions around native node ownership. */

import { resizeCubeFrame, type CubeResizeEdge } from '../cube/geometry/CubeResizeGeometry.js';
import type { CubeNode } from '../cube/node/ComfyCubeNodeFactory.js';
import type { Vec2 } from '../types/common.js';
import type { ComfyNode } from '../types/graph.js';
import type {
  ComfyLiteGraphWidgetInteraction,
  LiteGraphWidgetPointerSession,
} from './ComfyLiteGraphWidgetInteraction.js';
import {
  containsCubeCanvasPoint,
  type CubeCanvasLayout,
  type CubeCanvasPort,
  type CubeCanvasResizeHandle,
} from './CubeCanvasLayout.js';
import { computeCubeCanvasCardMenuLayout } from './CubeCanvasCardMenuLayout.js';
import {
  dispatchCubeFaceTitlebarAction,
  resolveCubeFaceTitlebarActions,
  type CubeFaceChromeActions,
} from './CubeFaceChromeActions.js';
import { requireCubeIdentity } from '../cube/node/ComfyCubeNodeFactory.js';

const BOUNDARY_PORT_HIT_RADIUS = 12;

export interface LiteGraphCubeNodeInteractionCanvas {
  canvas: HTMLCanvasElement;
  convertCanvasToOffset?(point: Vec2): unknown;
  ds?: { scale?: number; offset?: ArrayLike<number> };
  setDirty?(foreground?: boolean, background?: boolean): void;
}

export interface LiteGraphCubeNodeInteractionHistory {
  beforeChange?(): void;
  afterChange?(): void;
  setDirtyCanvas?(foreground?: boolean, background?: boolean): void;
}

export interface LiteGraphCubeNodeInteractionItem {
  node: CubeNode;
  layout: CubeCanvasLayout;
  cardMenuOpen: boolean;
}

export interface ComfyLiteGraphCubeNodeInteractionOptions {
  canvas: LiteGraphCubeNodeInteractionCanvas;
  history: LiteGraphCubeNodeInteractionHistory;
  widgetInteraction: ComfyLiteGraphWidgetInteraction;
  getItems(): readonly LiteGraphCubeNodeInteractionItem[];
  chromeActions?: CubeFaceChromeActions | null;
  onEdit(node: CubeNode): void;
  onCardMenuToggle(node: CubeNode): void;
  onCardRevealChange(node: CubeNode, internalNode: ComfyNode, revealed: boolean): void;
  onCardActivationChange(node: CubeNode, internalNode: ComfyNode, enabled: boolean): void;
}

type PointerSession =
  | {
      kind: 'resize';
      pointerId: number;
      node: CubeNode;
      edge: CubeResizeEdge;
      startPoint: Vec2;
      startPosition: Vec2;
      startSize: Vec2;
      minimumSize: Vec2;
    }
  | {
      kind: 'widget';
      pointerId: number;
      card: CubeCanvasLayout['cards'][number];
      session: LiteGraphWidgetPointerSession;
    };

/** Own custom face actions while native canvas code owns the parent node. */
export class ComfyLiteGraphCubeNodeInteraction {
  readonly #canvas: LiteGraphCubeNodeInteractionCanvas;
  readonly #history: LiteGraphCubeNodeInteractionHistory;
  readonly #widgetInteraction: ComfyLiteGraphWidgetInteraction;
  readonly #getItems: () => readonly LiteGraphCubeNodeInteractionItem[];
  readonly #chromeActions: CubeFaceChromeActions | null;
  readonly #onEdit: (node: CubeNode) => void;
  readonly #onCardMenuToggle: (node: CubeNode) => void;
  readonly #onCardRevealChange: (
    node: CubeNode,
    internalNode: ComfyNode,
    revealed: boolean,
  ) => void;
  readonly #onCardActivationChange: (
    node: CubeNode,
    internalNode: ComfyNode,
    enabled: boolean,
  ) => void;
  #session: PointerSession | null = null;

  /** Bind focused face hit testing ahead of LiteGraph's native node handlers. */
  constructor(options: ComfyLiteGraphCubeNodeInteractionOptions) {
    this.#canvas = options.canvas;
    this.#history = options.history;
    this.#widgetInteraction = options.widgetInteraction;
    this.#getItems = options.getItems;
    this.#chromeActions = options.chromeActions ?? null;
    this.#onEdit = options.onEdit;
    this.#onCardMenuToggle = options.onCardMenuToggle;
    this.#onCardRevealChange = options.onCardRevealChange;
    this.#onCardActivationChange = options.onCardActivationChange;
    options.canvas.canvas.addEventListener('pointerdown', this.#handlePointerDown, true);
    options.canvas.canvas.addEventListener('pointermove', this.#handlePointerMove, true);
    options.canvas.canvas.addEventListener('pointerup', this.#handlePointerUp, true);
    options.canvas.canvas.addEventListener('pointercancel', this.#handlePointerCancel, true);
    options.canvas.canvas.addEventListener('dblclick', this.#handleDoubleClick, true);
  }

  /** Release capture listeners and close any pending resize history transaction. */
  dispose(): void {
    if (this.#session?.kind === 'resize') this.#history.afterChange?.();
    this.#session = null;
    this.#canvas.canvas.removeEventListener('pointerdown', this.#handlePointerDown, true);
    this.#canvas.canvas.removeEventListener('pointermove', this.#handlePointerMove, true);
    this.#canvas.canvas.removeEventListener('pointerup', this.#handlePointerUp, true);
    this.#canvas.canvas.removeEventListener('pointercancel', this.#handlePointerCancel, true);
    this.#canvas.canvas.removeEventListener('dblclick', this.#handleDoubleClick, true);
  }

  /** Consume only Cube-owned actions, embedded widgets, and eight-way resizing. */
  readonly #handlePointerDown = (event: PointerEvent): void => {
    if (event.button !== 0) return;
    const point = this.#graphPoint(event);
    if (!point) return;
    const item = this.#itemAt(point);
    if (!item) return;
    if (item.cardMenuOpen) {
      const menuItem = computeCubeCanvasCardMenuLayout(item.layout).items.find((candidate) =>
        containsCubeCanvasPoint(candidate.rect, point),
      );
      if (menuItem) {
        const internalNode = item.node.subgraph._nodes.find(
          (candidate) => String(candidate.id ?? '') === menuItem.entry.id,
        );
        if (!internalNode) return;
        this.#consume(event);
        this.#onCardRevealChange(item.node, internalNode, !menuItem.entry.revealed);
        return;
      }
    }
    const resizeHandle = findResizeHandle(item.layout, point);
    if (resizeHandle) {
      this.#consume(event);
      this.#history.beforeChange?.();
      this.#session = {
        kind: 'resize',
        pointerId: event.pointerId,
        node: item.node,
        edge: resizeHandle.edge,
        startPoint: point,
        startPosition: [Number(item.node.pos[0]), Number(item.node.pos[1])],
        startSize: [Number(item.node.size[0]), Number(item.node.size[1])],
        minimumSize: [...item.layout.minimumSize],
      };
      this.#canvas.canvas.setPointerCapture?.(event.pointerId);
      return;
    }
    if (containsCubeCanvasPoint(item.layout.editAction, point)) {
      this.#consume(event);
      this.#onEdit(item.node);
      return;
    }
    if (
      item.layout.cardMenuEntries.length > 0 &&
      containsCubeCanvasPoint(item.layout.cardMenuAction, point)
    ) {
      this.#consume(event);
      this.#onCardMenuToggle(item.node);
      return;
    }
    for (const action of resolveCubeFaceTitlebarActions(
      requireCubeIdentity(item.node),
      this.#chromeActions,
    )) {
      const target = item.layout.chromeActions[action.key];
      if (target && containsCubeCanvasPoint(target, point)) {
        this.#consume(event);
        dispatchCubeFaceTitlebarAction(
          action.key,
          requireCubeIdentity(item.node),
          this.#chromeActions,
          event,
        );
        return;
      }
    }
    const card = item.layout.cards.find((candidate) =>
      containsCubeCanvasPoint(candidate.rect, point),
    );
    if (!card) return;
    if (card.activationAction && containsCubeCanvasPoint(card.activationAction, point)) {
      this.#consume(event);
      this.#onCardActivationChange(item.node, card.node, !card.enabled);
      return;
    }
    const session = this.#beginWidget(card, event, point);
    if (!session) return;
    this.#consume(event);
    this.#session = { kind: 'widget', pointerId: event.pointerId, card, session };
    this.#canvas.canvas.setPointerCapture?.(event.pointerId);
  };

  /** Update an active embedded widget or Cube-node resize. */
  readonly #handlePointerMove = (event: PointerEvent): void => {
    const point = this.#graphPoint(event);
    const session = this.#session;
    if (!session) {
      this.#syncCursor(point);
      return;
    }
    if (session.pointerId !== event.pointerId || !point) return;
    this.#consume(event);
    if (session.kind === 'widget') {
      const [x, y] = childGraphPoint(session.card, point);
      session.session.move(event, x, y);
      this.#markDirty();
      return;
    }
    const frame = resizeCubeFrame({
      edge: session.edge,
      startPosition: session.startPosition,
      startSize: session.startSize,
      delta: [point[0] - session.startPoint[0], point[1] - session.startPoint[1]],
      minimumSize: session.minimumSize,
    });
    writePair(session.node.pos, frame.position);
    session.node.setSize?.([...frame.size]);
    writePair(session.node.size, frame.size);
    session.node.onResize?.([...frame.size]);
    this.#markDirty();
  };

  /** Complete one focused pointer session. */
  readonly #handlePointerUp = (event: PointerEvent): void => {
    this.#finish(event, false);
  };

  /** Cancel one focused pointer session. */
  readonly #handlePointerCancel = (event: PointerEvent): void => {
    this.#finish(event, true);
  };

  /** Open Cube editing instead of Comfy's generic SubgraphNode double-click path. */
  readonly #handleDoubleClick = (event: MouseEvent): void => {
    const point = this.#graphPoint(event);
    if (!point) return;
    const item = this.#itemAt(point);
    if (!item || !containsCubeCanvasPoint(item.layout.header, point)) return;
    this.#consume(event);
    this.#onEdit(item.node);
  };

  /** Begin a native widget session in the real internal node's coordinates. */
  #beginWidget(
    card: CubeCanvasLayout['cards'][number],
    event: PointerEvent,
    point: Vec2,
  ): LiteGraphWidgetPointerSession | null {
    const [x, y] = childGraphPoint(card, point);
    return this.#widgetInteraction.begin(card.node, event, x, y, {
      width: card.rect.width,
      height: card.rect.height,
    });
  }

  /** Complete native widget or history lifecycle once. */
  #finish(event: PointerEvent, cancelled: boolean): void {
    const session = this.#session;
    if (!session || session.pointerId !== event.pointerId) return;
    const point = this.#graphPoint(event);
    this.#consume(event);
    if (session.kind === 'widget' && point) {
      const [x, y] = childGraphPoint(session.card, point);
      session.session.finish(event, x, y, cancelled);
    } else if (session.kind === 'resize') {
      this.#history.afterChange?.();
    }
    this.#canvas.canvas.releasePointerCapture?.(event.pointerId);
    this.#session = null;
    this.#markDirty();
    this.#syncCursor(point);
  }

  /** Find the topmost Cube whose real node contains one graph point. */
  #itemAt(point: Vec2): LiteGraphCubeNodeInteractionItem | null {
    return (
      [...this.#getItems()]
        .reverse()
        .find((candidate) => containsCubeCanvasPoint(candidate.layout.frame, point)) ?? null
    );
  }

  /** Convert viewport coordinates through Comfy's active graph transform. */
  #graphPoint(event: MouseEvent): Vec2 | null {
    const bounds = this.#canvas.canvas.getBoundingClientRect();
    const canvasPoint: Vec2 = [event.clientX - bounds.left, event.clientY - bounds.top];
    const converted = this.#canvas.convertCanvasToOffset?.(canvasPoint);
    if (Array.isArray(converted) && converted.length >= 2) {
      const point: Vec2 = [Number(converted[0]), Number(converted[1])];
      return point.every(Number.isFinite) ? point : null;
    }
    const scale = Number(this.#canvas.ds?.scale) || 1;
    const offset = this.#canvas.ds?.offset;
    const point: Vec2 = [
      canvasPoint[0] / scale - Number(offset?.[0] ?? 0),
      canvasPoint[1] / scale - Number(offset?.[1] ?? 0),
    ];
    return point.every(Number.isFinite) ? point : null;
  }

  /** Show the corresponding native resize cursor without owning selection. */
  #syncCursor(point: Vec2 | null): void {
    const handle = point ? findResizeHandle(this.#itemAt(point)?.layout ?? null, point) : null;
    this.#canvas.canvas.style.cursor = handle ? resizeCursor(handle.edge) : '';
  }

  /** Record one Cube-owned face interaction through the host history boundary. */
  #markDirty(): void {
    this.#history.setDirtyCanvas?.(true, true);
    this.#canvas.setDirty?.(true, true);
  }

  /** Prevent LiteGraph from also applying one Cube-owned interaction. */
  #consume(event: Event): void {
    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation();
  }
}

/** Resolve real child-node coordinates for one masonry card point. */
function childGraphPoint(card: CubeCanvasLayout['cards'][number], point: Vec2): Vec2 {
  const titleHeight = Math.max(1, card.rect.height - card.bodyHeight);
  return [
    Number(card.node.pos?.[0]) + point[0] - card.rect.x,
    Number(card.node.pos?.[1]) + point[1] - card.rect.y - titleHeight,
  ];
}

/** Return the most specific resize handle containing one graph point. */
function findResizeHandle(
  layout: CubeCanvasLayout | null,
  point: Vec2,
): CubeCanvasResizeHandle | null {
  if (!layout) return null;
  if (isBoundaryPortHit(layout, point)) return null;
  const corners = new Set<CubeResizeEdge>(['ne', 'se', 'sw', 'nw']);
  return (
    layout.resizeHandles.find(
      (handle) => corners.has(handle.edge) && containsCubeCanvasPoint(handle.rect, point),
    ) ??
    layout.resizeHandles.find((handle) => containsCubeCanvasPoint(handle.rect, point)) ??
    null
  );
}

/** Reserve native graph-link hit testing around every Cube boundary slot. */
function isBoundaryPortHit(layout: CubeCanvasLayout, point: Vec2): boolean {
  return [...layout.inputs, ...layout.outputs].some((port) => isPortHit(port, point));
}

/** Match Comfy's native slot affordance before considering Cube frame resizing. */
function isPortHit(port: CubeCanvasPort, point: Vec2): boolean {
  return (
    Math.abs(point[0] - port.x) <= BOUNDARY_PORT_HIT_RADIUS &&
    Math.abs(point[1] - port.y) <= BOUNDARY_PORT_HIT_RADIUS
  );
}

/** Map one resize direction to its platform cursor. */
function resizeCursor(edge: CubeResizeEdge): string {
  if (edge === 'n' || edge === 's') return 'ns-resize';
  if (edge === 'e' || edge === 'w') return 'ew-resize';
  if (edge === 'ne' || edge === 'sw') return 'nesw-resize';
  return 'nwse-resize';
}

/** Write a host-owned numeric vector without replacing its representation. */
function writePair(target: ArrayLike<number> & Record<number, number>, value: Vec2): void {
  target[0] = value[0];
  target[1] = value[1];
}
