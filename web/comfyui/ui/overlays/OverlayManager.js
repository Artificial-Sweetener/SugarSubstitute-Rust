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
 * Own the SugarCubes overlay rendering layer in `frontend/comfyui/ui/overlays/OverlayManager.js`.
 */
import { ProximityOverlay } from './ProximityOverlay.js';
import { PlacementOverlay } from './PlacementOverlay.js';
import { CubeChromeOverlay } from './CubeChromeOverlay.js';
import { createCubeSourceResolver } from './CubeSourceResolver.js';
import { ProximityPointerMoveTracker } from './proximity/ProximityPointerMoveTracker.js';
import { ProximityGraphMutationTracker } from './proximity/ProximityGraphMutationTracker.js';
import { OverlaySwapCoordinator } from './OverlaySwapCoordinator.js';
import { OverlayNodeMovementCoordinator } from './OverlayNodeMovementCoordinator.js';
import { OverlayGroupBoundsController } from './OverlayGroupBoundsController.js';
import { OverlayDrawHookLifecycle } from './OverlayDrawHookLifecycle.js';
import { OverlayGraphHookLifecycle } from './OverlayGraphHookLifecycle.js';
/**
 * Coordinate overlay manager behavior for the SugarCubes UI.
 */
export class OverlayManager {
    proximity;
    proximityPointerMoves;
    proximityGraphMutations;
    placement;
    swapCoordinator;
    chrome;
    drawHooks;
    graphHooks;
    nodeMovement;
    groupBounds;
    constructor({ adapter = null, events = null, scheduler = null, storage = null, cubeApi = null, cubeBrowser = null, saveService = null, saveDraft, toast = null, applyPreparedImport, reportImportOutcome, buildShiftedPlacementPayload, requestDirtyRefresh = null, layoutService = null, containmentService = null, collisionService = null, boundsReconciler = null, workflowLibraryState = null, workflowLibraryActions = null, } = {}) {
        this.proximity = new ProximityOverlay({
            adapter,
            events,
            scheduler,
            storage,
        });
        this.proximityPointerMoves = new ProximityPointerMoveTracker(this.proximity, scheduler);
        this.proximityGraphMutations = new ProximityGraphMutationTracker(this.proximity, scheduler);
        this.placement = new PlacementOverlay({
            adapter,
            events,
            scheduler,
            cubeApi,
            cubeBrowser,
            toast,
            ...(applyPreparedImport ? { applyPreparedImport } : {}),
            ...(reportImportOutcome ? { reportImportOutcome } : {}),
            ...(buildShiftedPlacementPayload ? { buildShiftedPlacementPayload } : {}),
        });
        this.swapCoordinator = new OverlaySwapCoordinator(adapter, layoutService, this.proximity);
        const saveImplementation = saveService?.saveImplementation?.bind(saveService);
        const chromeActions = {
            ...(saveImplementation
                ? {
                    onSaveImplementation: (metadata) => {
                        if (!metadata?.cube_id) {
                            return;
                        }
                        if (isHistoricalCubeMetadata(metadata)) {
                            toast?.push?.('warn', 'Historical version', 'Spawned historical versions cannot overwrite the current cube.');
                            return;
                        }
                        void saveImplementation({ cubeIds: [metadata.cube_id] });
                    },
                }
                : {}),
            ...(typeof saveDraft === 'function'
                ? {
                    onSaveDraft: (metadata) => {
                        const instanceId = typeof metadata.instance_id === 'string' ? metadata.instance_id : '';
                        if (metadata.kind === 'draft' && instanceId) {
                            void saveDraft(instanceId, metadata.graphSummary);
                        }
                    },
                }
                : {}),
            onSwapLeft: (metadata) => this.swapLayout(metadata, -1),
            onSwapRight: (metadata) => this.swapLayout(metadata, 1),
            canSwap: (metadata, direction) => this.canSwapDirection(metadata, direction),
            ...(workflowLibraryActions
                ? {
                    getLibraryClassification: (metadata) => workflowLibraryActions.classification(readInstanceId(metadata)),
                    onKeepWorkflowCube: (metadata) => workflowLibraryActions.keep(readInstanceId(metadata)),
                    onSaveWorkflowCubeToStable: (metadata) => {
                        void workflowLibraryActions.saveToStable(readInstanceId(metadata));
                    },
                    onSyncWorkflowCubeSource: (metadata) => {
                        void workflowLibraryActions.syncSource(readInstanceId(metadata));
                    },
                    onForkWorkflowCube: (metadata) => {
                        void workflowLibraryActions.forkToLocal(readInstanceId(metadata));
                    },
                }
                : {}),
        };
        this.chrome = new CubeChromeOverlay({
            adapter,
            actions: chromeActions,
            resolveSource: createCubeSourceResolver(cubeBrowser, workflowLibraryState),
        });
        this.drawHooks = new OverlayDrawHookLifecycle(adapter, scheduler, this.proximity, this.placement, this.chrome, this.proximityGraphMutations);
        this.nodeMovement = new OverlayNodeMovementCoordinator(adapter, scheduler, containmentService, collisionService, boundsReconciler, requestDirtyRefresh);
        this.groupBounds = new OverlayGroupBoundsController({
            adapter,
            containmentService,
            collisionService,
            requestDirtyRefresh,
            scheduleBoundsReconcile: (graph) => this.nodeMovement.scheduleBoundsReconcile(graph),
        });
        this.graphHooks = new OverlayGraphHookLifecycle({
            adapter,
            scheduler,
            target: this,
            proximity: this.proximity,
            placement: this.placement,
            chrome: this.chrome,
            proximityPointerMoves: this.proximityPointerMoves,
            proximityGraphMutations: this.proximityGraphMutations,
            events,
            requestDirtyRefresh,
            containmentService,
            collisionService,
        });
    }
    swapLayout(metadata, direction) {
        this.swapCoordinator.swapLayout(metadata, direction);
    }
    resolveLayoutOrigin(order) {
        return this.swapCoordinator.resolveLayoutOrigin(order);
    }
    resolveLayoutGaps(order) {
        return this.swapCoordinator.resolveLayoutGaps(order);
    }
    canSwapEntry(entry) {
        return this.swapCoordinator.canSwapEntry(entry);
    }
    canSwapDirection(metadata, direction) {
        return this.swapCoordinator.canSwapDirection(metadata, direction);
    }
    resolveProximityMatchesForSwap(graph) {
        return this.swapCoordinator.resolveProximityMatchesForSwap(graph);
    }
    resolveSwapPlan(graph, instanceId, direction) {
        return this.swapCoordinator.resolveSwapPlan(graph, instanceId, direction);
    }
    /** Return observable chrome state without exposing mutable overlay ownership. */
    getChromeDebugState() {
        return this.chrome.getDebugState();
    }
    setup() {
        this.chrome.setup();
        this.ensureOverlayHook();
        this.startOverlayWatchdog();
        this.ensureGraphHooks();
        this.ensureCollapseHook();
        this.ensureCleanHook();
    }
    dispose() {
        this.drawHooks.dispose();
        this.chrome?.dispose?.();
    }
    isOverlayHookActive(canvas) {
        return this.drawHooks.isOverlayHookActive(canvas);
    }
    ensureOverlayHook(attempt = 0) {
        this.drawHooks.ensureOverlayHook(attempt);
    }
    startOverlayWatchdog() {
        this.drawHooks.startOverlayWatchdog();
    }
    ensureCleanHook(attempt = 0) {
        this.graphHooks.ensureCleanHook(attempt);
    }
    ensureCollapseHook(attempt = 0) {
        this.graphHooks.ensureCollapseHook(attempt);
    }
    scheduleAfterFrames(callback, frames) {
        this.nodeMovement.scheduleAfterFrames(callback, frames);
    }
    clearGroupDragState() {
        this.groupBounds.clearGroupDragState();
    }
    readManagedGroupEntries(graph) {
        return this.groupBounds.readManagedGroupEntries(graph);
    }
    snapshotManagedGroupBounds(graph) {
        return this.groupBounds.snapshotManagedGroupBounds(graph);
    }
    boundsMatch(a, b) {
        return this.groupBounds.boundsMatch(a, b);
    }
    commitManagedGroupBoundsChanges(graph, previous = null) {
        return this.groupBounds.commitManagedGroupBoundsChanges(graph, previous);
    }
    resolveManagedCanvasGroup(canvas) {
        return this.groupBounds.resolveManagedCanvasGroup(canvas);
    }
    resolveManagedDragFallback() {
        return this.groupBounds.resolveManagedDragFallback();
    }
    captureGroupDragState(canvas) {
        this.groupBounds.captureGroupDragState(canvas);
    }
    commitGroupDrag(graph, canvas) {
        return this.groupBounds.commitGroupDrag(graph, canvas);
    }
    commitSelectedGroupBounds(graph, canvas) {
        return this.groupBounds.commitSelectedGroupBounds(graph, canvas);
    }
    onNodeCollapseToggled(options = {}) {
        this.nodeMovement.onNodeCollapseToggled(options);
    }
    ensureGraphHooks(attempt = 0) {
        this.graphHooks.ensureGraphHooks(attempt);
    }
    isMovableNodeCandidate(node) {
        return this.nodeMovement.isMovableNodeCandidate(node);
    }
    collectMovedNodes(node, canvas) {
        return this.nodeMovement.collectMovedNodes(node, canvas);
    }
    enqueueNodeMove(graph, node, canvas) {
        this.nodeMovement.enqueueNodeMove(graph, node, canvas);
    }
    enqueueNodes(graph, movedNodes, options = {}) {
        this.nodeMovement.enqueueNodes(graph, movedNodes, options);
    }
    flushNodeMoves(graph) {
        this.nodeMovement.flushNodeMoves(graph);
    }
    scheduleBoundsReconcile(graph) {
        this.nodeMovement.scheduleBoundsReconcile(graph);
    }
}
/** Require stable instance identity before dispatching a workflow-library action. */
function readInstanceId(metadata) {
    const instanceId = typeof metadata.instance_id === 'string' ? metadata.instance_id.trim() : '';
    if (!instanceId)
        throw new Error('Cube instance identity is unavailable.');
    return instanceId;
}
function isHistoricalCubeMetadata(metadata) {
    const revisionRef = typeof metadata?.cube_revision_ref === 'string' ? metadata.cube_revision_ref.trim() : '';
    return Boolean(revisionRef && revisionRef !== 'WORKTREE');
}
