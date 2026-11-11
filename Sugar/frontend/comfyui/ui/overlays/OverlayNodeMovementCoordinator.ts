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
 * Own node-movement queues, containment enforcement, and bounds reconciliation.
 */

import { isRecord } from '../types/common.js';
import type { ComfyCanvas, ComfyGraph, ComfyNode } from '../types/graph.js';
import type { CubeContainmentService } from '../layout/CubeContainmentService.js';
import type { CubeCollisionService } from '../layout/CubeCollisionService.js';
import type { CubeBoundsReconciler } from '../layout/CubeBoundsReconciler.js';

interface MovementAdapter {
  getApp?(): { graph?: ComfyGraph | null } | null;
}
interface MovementScheduler {
  raf?(callback: FrameRequestCallback): number | null;
}
interface MovementCanvas extends ComfyCanvas {
  selectedItems?: { values(): Iterable<unknown> };
}
type DirtyRefreshRequest = (options: { graph: ComfyGraph; reason: string }) => void;

/** Coordinate moved-node containment and post-layout reconciliation. */
export class OverlayNodeMovementCoordinator {
  private readonly nodeMoveQueue = new Map<string, ComfyNode[]>();
  private nodeMoveScheduled = false;
  private reconcileScheduled = false;
  private readonly expandContainmentRevisionByNodeId = new Map<string, number>();

  constructor(
    private readonly adapter: MovementAdapter | null,
    private readonly scheduler: MovementScheduler | null,
    private readonly containmentService: CubeContainmentService | null,
    private readonly collisionService: CubeCollisionService | null,
    private readonly boundsReconciler: CubeBoundsReconciler | null,
    private readonly requestDirtyRefresh: DirtyRefreshRequest | null,
  ) {}

  scheduleAfterFrames(callback: () => void, frames: number): void {
    const remaining = Number.isFinite(frames) ? Math.max(0, Math.floor(frames)) : 0;
    if (remaining <= 0 || typeof this.scheduler?.raf !== 'function') {
      callback();
      return;
    }
    this.scheduler.raf(() => this.scheduleAfterFrames(callback, remaining - 1));
  }

  onNodeCollapseToggled({
    node,
    wasCollapsed,
    isCollapsed,
  }: {
    node?: ComfyNode;
    wasCollapsed?: boolean;
    isCollapsed?: boolean;
  } = {}): void {
    if (!this.containmentService || !node || !wasCollapsed || isCollapsed) {
      return;
    }
    const graph = node?.graph || this.adapter?.getApp?.()?.graph || null;
    if (!graph) {
      return;
    }
    const nodeId = node?.id != null ? String(node.id) : '';
    if (!nodeId) {
      return;
    }
    const revision = (this.expandContainmentRevisionByNodeId.get(nodeId) || 0) + 1;
    this.expandContainmentRevisionByNodeId.set(nodeId, revision);
    this.scheduleAfterFrames(() => {
      if (this.expandContainmentRevisionByNodeId.get(nodeId) !== revision) {
        return;
      }
      if (
        (isRecord(node.flags) && node.flags.collapsed === true) ||
        !this.isMovableNodeCandidate(node)
      ) {
        return;
      }
      this.enqueueNodes(graph, [node], { schedule: false });
      this.flushNodeMoves(graph);
      this.scheduleBoundsReconcile(graph);
      this.requestDirtyRefresh?.({ graph, reason: 'node-expand' });
      graph?.setDirtyCanvas?.(true, true);
    }, 2);
  }

  isMovableNodeCandidate(node: unknown): node is ComfyNode & { id: string | number } {
    if (!isRecord(node) || node.id == null) {
      return false;
    }
    const pos = node.pos;
    const size = node.size;
    const hasPos =
      (Array.isArray(pos) || ArrayBuffer.isView(pos)) &&
      Number((pos as { length?: unknown }).length) >= 2;
    const hasSize =
      (Array.isArray(size) || ArrayBuffer.isView(size)) &&
      Number((size as { length?: unknown }).length) >= 2;
    return hasPos && hasSize;
  }

  collectMovedNodes(node: unknown, canvas: MovementCanvas | null | undefined): ComfyNode[] {
    const movedById = new Map<string, ComfyNode>();
    const addNode = (candidate: unknown): void => {
      if (!this.isMovableNodeCandidate(candidate)) {
        return;
      }
      const candidateId = String(candidate.id);
      if (!movedById.has(candidateId)) {
        movedById.set(candidateId, candidate);
      }
    };

    const selectedItems = canvas?.selectedItems;
    if (selectedItems?.values && typeof selectedItems.values === 'function') {
      for (const item of selectedItems.values()) {
        addNode(item);
      }
    }

    const legacySelected = canvas?.selected_nodes;
    if (legacySelected && typeof legacySelected === 'object') {
      for (const item of Object.values(legacySelected)) {
        addNode(item);
      }
    }

    if (movedById.size === 0) {
      addNode(node);
    }
    return Array.from(movedById.values());
  }

  enqueueNodeMove(
    graph: ComfyGraph,
    node: unknown,
    canvas: MovementCanvas | null | undefined,
  ): void {
    if (!this.containmentService) {
      return;
    }
    const movedNodes = this.collectMovedNodes(node, canvas);
    this.enqueueNodes(graph, movedNodes);
  }

  enqueueNodes(
    graph: ComfyGraph,
    movedNodes: readonly ComfyNode[],
    { schedule = true }: { schedule?: boolean } = {},
  ): void {
    if (!this.containmentService || !Array.isArray(movedNodes) || movedNodes.length === 0) {
      return;
    }
    for (const movedNode of movedNodes) {
      const nodeId = String(movedNode.id);
      const entry = this.nodeMoveQueue.get(nodeId) || [];
      entry.push(movedNode);
      this.nodeMoveQueue.set(nodeId, entry);
    }
    if (!schedule || this.nodeMoveScheduled) {
      return;
    }
    this.nodeMoveScheduled = true;
    this.scheduler?.raf?.(() => {
      this.nodeMoveScheduled = false;
      this.flushNodeMoves(graph);
    });
  }

  flushNodeMoves(graph: ComfyGraph): void {
    if (!this.containmentService || !this.collisionService) {
      this.nodeMoveQueue.clear();
      return;
    }
    const nodes: ComfyNode[] = [];
    for (const entries of this.nodeMoveQueue.values()) {
      nodes.push(...entries);
    }
    this.nodeMoveQueue.clear();
    if (!nodes.length) {
      return;
    }
    const index = this.containmentService.buildIndex(graph);
    const { instances } = this.containmentService.enforceForNodes({ graph, nodes, index });
    if (!instances || instances.size === 0) {
      return;
    }
    for (const instanceId of instances) {
      this.collisionService.resolveCollisions({ graph, activeInstanceId: instanceId, index });
    }
  }

  scheduleBoundsReconcile(graph: ComfyGraph): void {
    const boundsReconciler = this.boundsReconciler;
    if (!boundsReconciler || this.reconcileScheduled) {
      return;
    }
    this.reconcileScheduled = true;
    this.scheduler?.raf?.(() => {
      this.reconcileScheduled = false;
      const { changed, index } = boundsReconciler.reconcileAll({ graph });
      if (!changed || changed.size === 0 || !this.collisionService) {
        return;
      }
      for (const instanceId of changed) {
        this.collisionService.resolveCollisions({ graph, activeInstanceId: instanceId, index });
      }
    });
  }
}
