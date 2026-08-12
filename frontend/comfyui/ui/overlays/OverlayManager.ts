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
import type { ProximityMatch } from './ProximityOverlay.js';
import { PlacementOverlay } from './PlacementOverlay.js';
import { CubeChromeOverlay } from './CubeChromeOverlay.js';
import type { RectBounds, Vec2 } from '../types/common.js';
import type { ComfyGraph, ComfyNode } from '../types/graph.js';
import type { ChromeMetadata } from './CubeChromeOverlay.js';
import { createCubeSourceResolver } from './CubeSourceResolver.js';
import { ProximityPointerMoveTracker } from './proximity/ProximityPointerMoveTracker.js';
import { ProximityGraphMutationTracker } from './proximity/ProximityGraphMutationTracker.js';
import { OverlaySwapCoordinator } from './OverlaySwapCoordinator.js';
import { OverlayNodeMovementCoordinator } from './OverlayNodeMovementCoordinator.js';
import { OverlayGroupBoundsController } from './OverlayGroupBoundsController.js';
import { OverlayDrawHookLifecycle } from './OverlayDrawHookLifecycle.js';
import { OverlayGraphHookLifecycle } from './OverlayGraphHookLifecycle.js';
import type {
  CommittedGroupEntry,
  ManagedGroupEntry,
  ManagedGroupTarget,
  OverlayCanvas,
  OverlayManagerOptions,
  SwapEntry,
  SwapPlan,
} from './OverlayManagerContracts.js';

export type { OverlayManagerOptions } from './OverlayManagerContracts.js';

/**
 * Coordinate overlay manager behavior for the SugarCubes UI.
 */
export class OverlayManager {
  readonly proximity: ProximityOverlay;
  private readonly proximityPointerMoves: ProximityPointerMoveTracker;
  private readonly proximityGraphMutations: ProximityGraphMutationTracker;
  readonly placement: PlacementOverlay;
  private readonly swapCoordinator: OverlaySwapCoordinator;
  private readonly chrome: CubeChromeOverlay;
  private readonly drawHooks: OverlayDrawHookLifecycle;
  private readonly graphHooks: OverlayGraphHookLifecycle;
  private readonly nodeMovement: OverlayNodeMovementCoordinator;
  private readonly groupBounds: OverlayGroupBoundsController;

  constructor({
    adapter = null,
    events = null,
    scheduler = null,
    storage = null,
    cubeApi = null,
    cubeBrowser = null,
    saveService = null,
    saveDraft,
    toast = null,
    applyPreparedImport,
    reportImportOutcome,
    buildShiftedPlacementPayload,
    requestDirtyRefresh = null,
    layoutService = null,
    containmentService = null,
    collisionService = null,
    boundsReconciler = null,
  }: OverlayManagerOptions = {}) {
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
            onSaveImplementation: (metadata: ChromeMetadata) => {
              if (!metadata?.cube_id) {
                return;
              }
              if (isHistoricalCubeMetadata(metadata)) {
                toast?.push?.(
                  'warn',
                  'Historical version',
                  'Spawned historical versions cannot overwrite the current cube.',
                );
                return;
              }
              void saveImplementation({ cubeIds: [metadata.cube_id] });
            },
          }
        : {}),
      ...(typeof saveDraft === 'function'
        ? {
            onSaveDraft: (metadata: ChromeMetadata) => {
              const instanceId =
                typeof metadata.instance_id === 'string' ? metadata.instance_id : '';
              if (metadata.kind === 'draft' && instanceId) {
                void saveDraft(instanceId, metadata.graphSummary);
              }
            },
          }
        : {}),
      onSwapLeft: (metadata: ChromeMetadata) => this.swapLayout(metadata, -1),
      onSwapRight: (metadata: ChromeMetadata) => this.swapLayout(metadata, 1),
      canSwap: (metadata: ChromeMetadata, direction: 'left' | 'right') =>
        this.canSwapDirection(metadata, direction),
    };
    this.chrome = new CubeChromeOverlay({
      adapter,
      actions: chromeActions,
      resolveSource: createCubeSourceResolver(cubeBrowser),
    });
    this.drawHooks = new OverlayDrawHookLifecycle(
      adapter,
      scheduler,
      this.proximity,
      this.placement,
      this.chrome,
      this.proximityGraphMutations,
    );
    this.nodeMovement = new OverlayNodeMovementCoordinator(
      adapter,
      scheduler,
      containmentService,
      collisionService,
      boundsReconciler,
      requestDirtyRefresh,
    );
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

  swapLayout(metadata: ChromeMetadata, direction: -1 | 1): void {
    this.swapCoordinator.swapLayout(metadata, direction);
  }

  resolveLayoutOrigin(order: readonly SwapEntry[]): Vec2 {
    return this.swapCoordinator.resolveLayoutOrigin(order);
  }

  resolveLayoutGaps(order: readonly SwapEntry[]): number[] {
    return this.swapCoordinator.resolveLayoutGaps(order);
  }

  canSwapEntry(entry: SwapEntry | null | undefined): boolean {
    return this.swapCoordinator.canSwapEntry(entry);
  }

  canSwapDirection(metadata: ChromeMetadata, direction: 'left' | 'right' | -1 | 1): boolean {
    return this.swapCoordinator.canSwapDirection(metadata, direction);
  }

  resolveProximityMatchesForSwap(graph: ComfyGraph): ProximityMatch[] {
    return this.swapCoordinator.resolveProximityMatchesForSwap(graph);
  }

  resolveSwapPlan(graph: ComfyGraph, instanceId: string, direction: -1 | 1): SwapPlan | null {
    return this.swapCoordinator.resolveSwapPlan(graph, instanceId, direction);
  }

  /** Return observable chrome state without exposing mutable overlay ownership. */
  getChromeDebugState() {
    return this.chrome.getDebugState();
  }

  setup(): void {
    this.chrome.setup();
    this.ensureOverlayHook();
    this.startOverlayWatchdog();
    this.ensureGraphHooks();
    this.ensureCollapseHook();
    this.ensureCleanHook();
  }

  dispose(): void {
    this.drawHooks.dispose();
    this.chrome?.dispose?.();
  }

  isOverlayHookActive(canvas: OverlayCanvas | null | undefined): boolean {
    return this.drawHooks.isOverlayHookActive(canvas);
  }

  ensureOverlayHook(attempt = 0): void {
    this.drawHooks.ensureOverlayHook(attempt);
  }

  startOverlayWatchdog(): void {
    this.drawHooks.startOverlayWatchdog();
  }

  ensureCleanHook(attempt = 0): void {
    this.graphHooks.ensureCleanHook(attempt);
  }

  ensureCollapseHook(attempt = 0): void {
    this.graphHooks.ensureCollapseHook(attempt);
  }

  scheduleAfterFrames(callback: () => void, frames: number): void {
    this.nodeMovement.scheduleAfterFrames(callback, frames);
  }

  clearGroupDragState(): void {
    this.groupBounds.clearGroupDragState();
  }

  readManagedGroupEntries(graph: ComfyGraph | null | undefined): ManagedGroupEntry[] {
    return this.groupBounds.readManagedGroupEntries(graph);
  }

  snapshotManagedGroupBounds(graph: ComfyGraph): Map<string, RectBounds> {
    return this.groupBounds.snapshotManagedGroupBounds(graph);
  }

  boundsMatch(a: unknown, b: RectBounds): boolean {
    return this.groupBounds.boundsMatch(a, b);
  }

  commitManagedGroupBoundsChanges(
    graph: ComfyGraph,
    previous: ReadonlyMap<string, RectBounds> | null = null,
  ): CommittedGroupEntry[] {
    return this.groupBounds.commitManagedGroupBoundsChanges(graph, previous);
  }

  resolveManagedCanvasGroup(canvas: OverlayCanvas | null | undefined): ManagedGroupTarget | null {
    return this.groupBounds.resolveManagedCanvasGroup(canvas);
  }

  resolveManagedDragFallback(): ManagedGroupTarget | null {
    return this.groupBounds.resolveManagedDragFallback();
  }

  captureGroupDragState(canvas: OverlayCanvas): void {
    this.groupBounds.captureGroupDragState(canvas);
  }

  commitGroupDrag(
    graph: ComfyGraph,
    canvas: OverlayCanvas | null | undefined,
  ): CommittedGroupEntry | null {
    return this.groupBounds.commitGroupDrag(graph, canvas);
  }

  commitSelectedGroupBounds(graph: ComfyGraph, canvas: OverlayCanvas): CommittedGroupEntry | null {
    return this.groupBounds.commitSelectedGroupBounds(graph, canvas);
  }

  onNodeCollapseToggled(
    options: {
      node?: ComfyNode;
      wasCollapsed?: boolean;
      isCollapsed?: boolean;
    } = {},
  ): void {
    this.nodeMovement.onNodeCollapseToggled(options);
  }

  ensureGraphHooks(attempt = 0): void {
    this.graphHooks.ensureGraphHooks(attempt);
  }

  isMovableNodeCandidate(node: unknown): node is ComfyNode & { id: string | number } {
    return this.nodeMovement.isMovableNodeCandidate(node);
  }

  collectMovedNodes(node: unknown, canvas: OverlayCanvas | null | undefined): ComfyNode[] {
    return this.nodeMovement.collectMovedNodes(node, canvas);
  }

  enqueueNodeMove(
    graph: ComfyGraph,
    node: unknown,
    canvas: OverlayCanvas | null | undefined,
  ): void {
    this.nodeMovement.enqueueNodeMove(graph, node, canvas);
  }

  enqueueNodes(
    graph: ComfyGraph,
    movedNodes: readonly ComfyNode[],
    options: { schedule?: boolean } = {},
  ): void {
    this.nodeMovement.enqueueNodes(graph, movedNodes, options);
  }

  flushNodeMoves(graph: ComfyGraph): void {
    this.nodeMovement.flushNodeMoves(graph);
  }

  scheduleBoundsReconcile(graph: ComfyGraph): void {
    this.nodeMovement.scheduleBoundsReconcile(graph);
  }
}

function isHistoricalCubeMetadata(metadata: ChromeMetadata): boolean {
  const revisionRef =
    typeof metadata?.cube_revision_ref === 'string' ? metadata.cube_revision_ref.trim() : '';
  return Boolean(revisionRef && revisionRef !== 'WORKTREE');
}
