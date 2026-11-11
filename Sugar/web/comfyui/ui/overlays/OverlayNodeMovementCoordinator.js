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
/** Coordinate moved-node containment and post-layout reconciliation. */
export class OverlayNodeMovementCoordinator {
    adapter;
    scheduler;
    containmentService;
    collisionService;
    boundsReconciler;
    requestDirtyRefresh;
    nodeMoveQueue = new Map();
    nodeMoveScheduled = false;
    reconcileScheduled = false;
    expandContainmentRevisionByNodeId = new Map();
    constructor(adapter, scheduler, containmentService, collisionService, boundsReconciler, requestDirtyRefresh) {
        this.adapter = adapter;
        this.scheduler = scheduler;
        this.containmentService = containmentService;
        this.collisionService = collisionService;
        this.boundsReconciler = boundsReconciler;
        this.requestDirtyRefresh = requestDirtyRefresh;
    }
    scheduleAfterFrames(callback, frames) {
        const remaining = Number.isFinite(frames) ? Math.max(0, Math.floor(frames)) : 0;
        if (remaining <= 0 || typeof this.scheduler?.raf !== 'function') {
            callback();
            return;
        }
        this.scheduler.raf(() => this.scheduleAfterFrames(callback, remaining - 1));
    }
    onNodeCollapseToggled({ node, wasCollapsed, isCollapsed, } = {}) {
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
            if ((isRecord(node.flags) && node.flags.collapsed === true) ||
                !this.isMovableNodeCandidate(node)) {
                return;
            }
            this.enqueueNodes(graph, [node], { schedule: false });
            this.flushNodeMoves(graph);
            this.scheduleBoundsReconcile(graph);
            this.requestDirtyRefresh?.({ graph, reason: 'node-expand' });
            graph?.setDirtyCanvas?.(true, true);
        }, 2);
    }
    isMovableNodeCandidate(node) {
        if (!isRecord(node) || node.id == null) {
            return false;
        }
        const pos = node.pos;
        const size = node.size;
        const hasPos = (Array.isArray(pos) || ArrayBuffer.isView(pos)) &&
            Number(pos.length) >= 2;
        const hasSize = (Array.isArray(size) || ArrayBuffer.isView(size)) &&
            Number(size.length) >= 2;
        return hasPos && hasSize;
    }
    collectMovedNodes(node, canvas) {
        const movedById = new Map();
        const addNode = (candidate) => {
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
    enqueueNodeMove(graph, node, canvas) {
        if (!this.containmentService) {
            return;
        }
        const movedNodes = this.collectMovedNodes(node, canvas);
        this.enqueueNodes(graph, movedNodes);
    }
    enqueueNodes(graph, movedNodes, { schedule = true } = {}) {
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
    flushNodeMoves(graph) {
        if (!this.containmentService || !this.collisionService) {
            this.nodeMoveQueue.clear();
            return;
        }
        const nodes = [];
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
    scheduleBoundsReconcile(graph) {
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
