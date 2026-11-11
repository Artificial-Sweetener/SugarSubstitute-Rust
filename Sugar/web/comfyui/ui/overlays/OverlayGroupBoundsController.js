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
 * Own managed-group drag capture and canonical bounds commits.
 */
import { readGroupBounds } from '../graph/Bounds.js';
import { writeCanonicalBounds } from '../graph/CubeBounds.js';
import { isRecord } from '../types/common.js';
function readManagedMetadata(group) {
    const sugarcubes = isRecord(group?.properties) ? group.properties.sugarcubes : null;
    if (!isRecord(sugarcubes) || sugarcubes.managed !== true || !sugarcubes.instance_id)
        return null;
    return sugarcubes;
}
/** Reconcile managed group geometry at host interaction boundaries. */
export class OverlayGroupBoundsController {
    options;
    groupDragState = null;
    adapter;
    containmentService;
    collisionService;
    requestDirtyRefresh;
    constructor(options) {
        this.options = options;
        this.adapter = options.adapter;
        this.containmentService = options.containmentService;
        this.collisionService = options.collisionService;
        this.requestDirtyRefresh = options.requestDirtyRefresh;
    }
    clearGroupDragState() {
        this.groupDragState = null;
    }
    readManagedGroupEntries(graph) {
        const groups = Array.isArray(graph?._groups) ? graph._groups : graph?.groups || [];
        const entries = [];
        for (const group of groups) {
            const metadata = readManagedMetadata(group);
            if (!metadata) {
                continue;
            }
            const bounds = readGroupBounds(group);
            if (!bounds) {
                continue;
            }
            entries.push({
                instanceId: String(metadata.instance_id),
                group,
                metadata,
                bounds: {
                    x: bounds[0],
                    y: bounds[1],
                    w: bounds[2],
                    h: bounds[3],
                },
            });
        }
        return entries;
    }
    snapshotManagedGroupBounds(graph) {
        const snapshot = new Map();
        for (const entry of this.readManagedGroupEntries(graph)) {
            snapshot.set(entry.instanceId, { ...entry.bounds });
        }
        return snapshot;
    }
    boundsMatch(a, b) {
        if (!isRecord(a)) {
            return false;
        }
        return a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h;
    }
    commitManagedGroupBoundsChanges(graph, previous = null) {
        const entries = this.readManagedGroupEntries(graph);
        const committed = [];
        for (const entry of entries) {
            const prior = previous?.get?.(entry.instanceId) || null;
            const metadataBounds = entry.metadata?.bounds || null;
            const metadataMatches = this.boundsMatch(metadataBounds, entry.bounds);
            const changedSinceSnapshot = !prior || !this.boundsMatch(prior, entry.bounds);
            if (metadataMatches && !changedSinceSnapshot) {
                continue;
            }
            const canonicalBounds = entry.metadata.bounds
                ? {
                    ...entry.bounds,
                    padding: entry.metadata.bounds.padding,
                    header: entry.metadata.bounds.header,
                }
                : { ...entry.bounds };
            writeCanonicalBounds({
                group: entry.group,
                metadata: entry.metadata,
                bounds: canonicalBounds,
            });
            committed.push({
                group: entry.group,
                metadata: entry.metadata,
                bounds: canonicalBounds,
            });
        }
        return committed;
    }
    resolveManagedCanvasGroup(canvas) {
        const candidates = [];
        const addCandidate = (group) => {
            if (isRecord(group)) {
                candidates.push(group);
            }
        };
        addCandidate(canvas?.selected_group);
        addCandidate(canvas?.resizingGroup);
        if (canvas?.selected_group_resizing && typeof canvas?.selected_group_resizing === 'object') {
            addCandidate(canvas.selected_group_resizing);
        }
        const seen = new Set();
        for (const group of candidates) {
            if (seen.has(group)) {
                continue;
            }
            seen.add(group);
            const metadata = readManagedMetadata(group);
            if (metadata) {
                return { group, metadata };
            }
        }
        return null;
    }
    resolveManagedDragFallback() {
        const state = this.groupDragState;
        if (!state?.group) {
            return null;
        }
        const group = state.group;
        const metadata = readManagedMetadata(group);
        if (!metadata || metadata.instance_id !== state.instanceId) {
            return null;
        }
        const bounds = readGroupBounds(group);
        if (!bounds) {
            return null;
        }
        const changedFromCapture = bounds[0] !== state.bounds.x ||
            bounds[1] !== state.bounds.y ||
            bounds[2] !== state.bounds.w ||
            bounds[3] !== state.bounds.h;
        if (!changedFromCapture) {
            return null;
        }
        return { group, metadata, bounds };
    }
    captureGroupDragState(canvas) {
        const target = this.resolveManagedCanvasGroup(canvas);
        if (!target) {
            this.clearGroupDragState();
            return;
        }
        const { group, metadata } = target;
        const bounds = readGroupBounds(group);
        if (!bounds) {
            this.clearGroupDragState();
            return;
        }
        this.groupDragState = {
            group,
            instanceId: metadata.instance_id,
            bounds: {
                x: bounds[0],
                y: bounds[1],
                w: bounds[2],
                h: bounds[3],
            },
        };
    }
    commitGroupDrag(graph, canvas) {
        const state = this.groupDragState;
        this.clearGroupDragState();
        if (!state?.group) {
            return null;
        }
        const group = state.group;
        const metadata = readManagedMetadata(group);
        if (!metadata || metadata.instance_id !== state.instanceId) {
            return null;
        }
        const bounds = readGroupBounds(group);
        if (!bounds) {
            return null;
        }
        const nextBounds = {
            x: bounds[0],
            y: bounds[1],
            w: bounds[2],
            h: bounds[3],
        };
        const changed = nextBounds.x !== state.bounds.x ||
            nextBounds.y !== state.bounds.y ||
            nextBounds.w !== state.bounds.w ||
            nextBounds.h !== state.bounds.h;
        if (!changed) {
            return null;
        }
        const canonicalBounds = metadata.bounds
            ? {
                ...nextBounds,
                padding: metadata.bounds.padding,
                header: metadata.bounds.header,
            }
            : nextBounds;
        writeCanonicalBounds({
            group,
            metadata,
            bounds: canonicalBounds,
        });
        if (this.containmentService && this.collisionService && graph) {
            const index = this.containmentService.buildIndex(graph);
            if (metadata.instance_id) {
                this.collisionService.resolveCollisions({
                    graph,
                    activeInstanceId: metadata.instance_id,
                    index,
                });
            }
        }
        this.options.scheduleBoundsReconcile(graph);
        this.requestDirtyRefresh?.({ graph, reason: 'group-move' });
        graph?.setDirtyCanvas?.(true, true);
        const canvasRef = canvas || this.adapter?.getApp?.()?.canvas || null;
        canvasRef?.setDirty?.(true, true);
        return {
            group,
            metadata,
            bounds: canonicalBounds,
        };
    }
    commitSelectedGroupBounds(_graph, canvas) {
        const target = this.resolveManagedCanvasGroup(canvas) || this.resolveManagedDragFallback();
        if (!target) {
            return null;
        }
        const { group, metadata } = target;
        const bounds = target.bounds || readGroupBounds(group);
        if (!bounds || !metadata?.managed || !metadata?.instance_id) {
            return null;
        }
        const canonicalBounds = metadata.bounds
            ? {
                x: bounds[0],
                y: bounds[1],
                w: bounds[2],
                h: bounds[3],
                padding: metadata.bounds.padding,
                header: metadata.bounds.header,
            }
            : {
                x: bounds[0],
                y: bounds[1],
                w: bounds[2],
                h: bounds[3],
            };
        writeCanonicalBounds({ group, metadata, bounds: canonicalBounds });
        if (this.groupDragState?.group === group) {
            this.clearGroupDragState();
        }
        return { group, metadata, bounds: canonicalBounds };
    }
}
