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
 * Define overlay orchestration collaboration boundaries.
 */

import type { ProximityMatch, ProximityOverlay } from './ProximityOverlay.js';
import type { PlacementOverlay } from './PlacementOverlay.js';
import type { Bounds, RectBounds, UnknownRecord, Vec2 } from '../types/common.js';
import type { ComfyCanvas, ComfyGraph, ComfyGroup } from '../types/graph.js';
import type { CubeGroupMetadataRecord } from '../graph/GroupMetadata.js';
import type { LayoutEntry } from '../layout/CubeLayoutEngine.js';
import type { CubeContainmentService } from '../layout/CubeContainmentService.js';
import type { CubeCollisionService } from '../layout/CubeCollisionService.js';
import type { CubeBoundsReconciler } from '../layout/CubeBoundsReconciler.js';
import type { CubeFaceGraphSummary } from '../surface/CubeFaceChromeActions.js';
import type { CubeWorkflowLibraryState } from '../workflow/CubeWorkflowLibraryState.js';
import type { CubeWorkflowLibraryActions } from '../workflow/CubeWorkflowLibraryActions.js';

type PlacementOptions = NonNullable<ConstructorParameters<typeof PlacementOverlay>[0]>;

export interface OverlayAdapter {
  getApp?(): ReturnType<NonNullable<NonNullable<PlacementOptions['adapter']>['getApp']>>;
  getWindow?(): Window | null;
  getConsole?(): Console | null;
  getLiteGraph?(): LiteGraphHost | null;
}
export interface OverlayScheduler {
  raf?(callback: FrameRequestCallback): number | null;
  timeout?(callback: () => void, delayMs: number): number | null;
}
export interface OverlayEvents {
  emit?<T>(name: string, detail: T): void;
}
interface SaveService {
  saveImplementation?(options: { cubeIds: string[] }): unknown;
}
interface DraftSaveService {
  saveDraft?(instanceId: string, graphSummary?: CubeFaceGraphSummary): unknown;
}
interface ToastService {
  push?(severity: string, summary: string, detail: string): void;
}
interface CubeCatalog {
  getCubeById?(cubeId: string): unknown;
}
export interface LayoutCoordinator {
  buildIndex(graph: ComfyGraph): unknown;
  deriveOrder(
    index: unknown,
    options?: {
      graph?: ComfyGraph;
      anchorInstanceId?: string;
      proximityMatches?: readonly ProximityMatch[];
    },
  ): LayoutEntry[];
  swapOrder(options: {
    graph: ComfyGraph;
    aId: string;
    bId: string;
    order: LayoutEntry[];
    layout: { origin: Vec2; gaps: number[]; minGap: number };
  }): unknown;
}
export interface OverlayManagerOptions {
  adapter?: OverlayAdapter | null;
  events?: OverlayEvents | null;
  scheduler?: OverlayScheduler | null;
  storage?: NonNullable<ConstructorParameters<typeof ProximityOverlay>[0]>['storage'];
  cubeApi?: PlacementOptions['cubeApi'];
  cubeBrowser?: (PlacementOptions['cubeBrowser'] & CubeCatalog) | null;
  saveService?: SaveService | null;
  saveDraft?: DraftSaveService['saveDraft'];
  toast?: (PlacementOptions['toast'] & ToastService) | null;
  applyPreparedImport?: PlacementOptions['applyPreparedImport'];
  reportImportOutcome?: PlacementOptions['reportImportOutcome'];
  buildShiftedPlacementPayload?: PlacementOptions['buildShiftedPlacementPayload'];
  requestDirtyRefresh?: DirtyRefreshRequest | null;
  layoutService?: LayoutCoordinator | null;
  containmentService?: CubeContainmentService | null;
  collisionService?: CubeCollisionService | null;
  boundsReconciler?: CubeBoundsReconciler | null;
  workflowLibraryState?: CubeWorkflowLibraryState | null;
  workflowLibraryActions?: CubeWorkflowLibraryActions | null;
}
export interface ManagedMetadata extends CubeGroupMetadataRecord {
  managed: true;
  instance_id: string;
  bounds?: UnknownRecord;
}
export interface ManagedGroupEntry {
  instanceId: string;
  group: ComfyGroup;
  metadata: ManagedMetadata;
  bounds: RectBounds;
}
export interface CommittedGroupEntry {
  group: ComfyGroup;
  metadata: ManagedMetadata;
  bounds: RectBounds & UnknownRecord;
}
export interface ManagedGroupTarget {
  group: ComfyGroup;
  metadata: ManagedMetadata;
  bounds?: Bounds;
}
export interface OverlayCanvas extends ComfyCanvas {
  graph: ComfyGraph;
  selected_group?: ComfyGroup | null;
  resizingGroup?: ComfyGroup | null;
  selected_group_resizing?: ComfyGroup | boolean | null;
  selectedItems?: { values(): Iterable<unknown> };
  onDrawForeground?: HookedDrawCallback | null;
  onDrawBackground?: HookedDrawCallback | null;
  onAfterChange?: (this: OverlayCanvas, ...args: unknown[]) => unknown;
  onNodeMoved?: (this: OverlayCanvas, node: unknown, ...args: unknown[]) => unknown;
  processMouseMove?: (this: OverlayCanvas, ...args: unknown[]) => unknown;
  processMouseDown?: (
    this: OverlayCanvas,
    event: MouseEvent | PointerEvent,
    ...args: unknown[]
  ) => unknown;
  processMouseUp?: (this: OverlayCanvas, ...args: unknown[]) => unknown;
}
interface HookedDrawCallback {
  (this: OverlayCanvas, ctx: CanvasRenderingContext2D, ...args: unknown[]): unknown;
  __sugarcubes_overlay_hooked?: boolean;
}
export type DirtyRefreshRequest = (options: { graph: ComfyGraph; reason: string }) => void;
export interface SwapEntry extends LayoutEntry {
  bounds?: RectBounds | null;
  markerLookup?: { inputs?: unknown[]; outputs?: unknown[] };
}
export interface SwapPlan {
  order: SwapEntry[];
  current: SwapEntry;
  neighbor: SwapEntry;
}
