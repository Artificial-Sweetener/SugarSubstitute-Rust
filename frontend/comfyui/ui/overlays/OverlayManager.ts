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
import { readGroupBounds } from '../graph/Bounds.js';
import { writeCanonicalBounds } from '../graph/CubeBounds.js';
import { isRecord } from '../types/common.js';
import type { Bounds, RectBounds, UnknownRecord, Vec2 } from '../types/common.js';
import type { ComfyCanvas, ComfyGraph, ComfyGroup, ComfyNode } from '../types/graph.js';
import type { CubeGroupMetadataRecord } from '../graph/GroupMetadata.js';
import type { LayoutEntry } from '../layout/CubeLayoutEngine.js';
import type { CubeContainmentService } from '../layout/CubeContainmentService.js';
import type { CubeCollisionService } from '../layout/CubeCollisionService.js';
import type { CubeBoundsReconciler } from '../layout/CubeBoundsReconciler.js';
import type { ChromeMetadata } from './CubeChromeOverlay.js';
import { createCubeSourceResolver } from './CubeSourceResolver.js';

type PlacementOptions = NonNullable<ConstructorParameters<typeof PlacementOverlay>[0]>;

interface OverlayAdapter {
  getApp?(): ReturnType<NonNullable<NonNullable<PlacementOptions['adapter']>['getApp']>>;
  getWindow?(): Window | null;
  getConsole?(): Console | null;
  getLiteGraph?(): LiteGraphHost | null;
}

interface OverlayScheduler {
  raf?(callback: FrameRequestCallback): number | null;
  timeout?(callback: () => void, delayMs: number): number | null;
}

interface OverlayEvents {
  emit?<T>(name: string, detail: T): void;
}

interface SaveService {
  saveImplementation?(options: { cubeIds: string[] }): unknown;
}

interface FlavorService {
  saveCurrentFaceValuesAsCubeDefaults?: unknown;
}

interface ToastService {
  push?(severity: string, summary: string, detail: string): void;
}

interface CubeCatalog {
  getCubeById?(cubeId: string): unknown;
}

interface LayoutCoordinator {
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
  api?: NonNullable<ConstructorParameters<typeof ProximityOverlay>[0]>['api'];
  cubeApi?: PlacementOptions['cubeApi'];
  cubeBrowser?: (PlacementOptions['cubeBrowser'] & CubeCatalog) | null;
  saveService?: SaveService | null;
  flavorService?: FlavorService | null;
  toast?: (PlacementOptions['toast'] & ToastService) | null;
  applyPreparedImport?: PlacementOptions['applyPreparedImport'];
  reportImportOutcome?: PlacementOptions['reportImportOutcome'];
  buildShiftedPlacementPayload?: PlacementOptions['buildShiftedPlacementPayload'];
  requestDirtyRefresh?: DirtyRefreshRequest | null;
  layoutService?: LayoutCoordinator | null;
  containmentService?: CubeContainmentService | null;
  collisionService?: CubeCollisionService | null;
  boundsReconciler?: CubeBoundsReconciler | null;
}

interface ManagedMetadata extends CubeGroupMetadataRecord {
  managed: true;
  instance_id: string;
  bounds?: UnknownRecord;
}

interface ManagedGroupEntry {
  instanceId: string;
  group: ComfyGroup;
  metadata: ManagedMetadata;
  bounds: RectBounds;
}

interface CommittedGroupEntry {
  group: ComfyGroup;
  metadata: ManagedMetadata;
  bounds: RectBounds & UnknownRecord;
}

interface GroupDragState {
  group: ComfyGroup;
  instanceId: string;
  bounds: RectBounds;
}

interface ManagedGroupTarget {
  group: ComfyGroup;
  metadata: ManagedMetadata;
  bounds?: Bounds;
}

interface OverlayCanvas extends ComfyCanvas {
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

interface HookedCanvasElement extends HTMLCanvasElement {
  __sugarcubes_chrome_listener?: (event: MouseEvent | PointerEvent) => boolean;
  __sugarcubes_chrome_move_listener?: (event: MouseEvent | PointerEvent) => void;
}

interface HookedDrawCallback {
  (this: OverlayCanvas, ctx: CanvasRenderingContext2D, ...args: unknown[]): unknown;
  __sugarcubes_overlay_hooked?: boolean;
}

interface MutablePrototype extends UnknownRecord {
  drawConnections?: (
    this: OverlayCanvas,
    ctx: CanvasRenderingContext2D,
    ...args: unknown[]
  ) => unknown;
  drawForeground?: (
    this: OverlayCanvas,
    ctx: CanvasRenderingContext2D,
    ...args: unknown[]
  ) => unknown;
  collapse?: HostCallback;
}

type HostCallback = (this: UnknownRecord, ...args: unknown[]) => unknown;
type DirtyRefreshRequest = (options: { graph: ComfyGraph; reason: string }) => void;

interface SwapEntry extends LayoutEntry {
  bounds?: RectBounds | null;
  markerLookup?: { inputs?: unknown[]; outputs?: unknown[] };
}

interface SwapPlan {
  order: SwapEntry[];
  current: SwapEntry;
  neighbor: SwapEntry;
}

function readManagedMetadata(group: ComfyGroup | null | undefined): ManagedMetadata | null {
  const sugarcubes = isRecord(group?.properties) ? group.properties.sugarcubes : null;
  if (!isRecord(sugarcubes) || sugarcubes.managed !== true || !sugarcubes.instance_id) {
    return null;
  }
  return sugarcubes as ManagedMetadata;
}

/**
 * Coordinate overlay manager behavior for the SugarCubes UI.
 */
export class OverlayManager {
  private readonly adapter: OverlayAdapter | null;
  private readonly events: OverlayEvents | null;
  private readonly scheduler: OverlayScheduler | null;
  readonly proximity: ProximityOverlay;
  readonly placement: PlacementOverlay;
  private readonly layoutService: LayoutCoordinator | null;
  private readonly chrome: CubeChromeOverlay;
  private overlayDrawHooked: boolean;
  private readonly overlayWatchdog: {
    timerId: number | null;
    attempts: number;
  };
  private cleanHooked: boolean;
  private graphHooksWrapped: boolean;
  private readonly requestDirtyRefresh: DirtyRefreshRequest | null;
  private readonly containmentService: CubeContainmentService | null;
  private readonly collisionService: CubeCollisionService | null;
  private readonly boundsReconciler: CubeBoundsReconciler | null;
  private readonly nodeMoveQueue: Map<string, ComfyNode[]>;
  private nodeMoveScheduled: boolean;
  private reconcileScheduled: boolean;
  private readonly expandContainmentRevisionByNodeId: Map<string, number>;
  private groupDragState: GroupDragState | null;

  constructor({
    adapter = null,
    events = null,
    scheduler = null,
    storage = null,
    api = null,
    cubeApi = null,
    cubeBrowser = null,
    saveService = null,
    flavorService = null,
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
    this.adapter = adapter;
    this.events = events;
    this.scheduler = scheduler;
    this.proximity = new ProximityOverlay({
      adapter,
      events,
      scheduler,
      storage,
      api,
    });
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
    this.layoutService = layoutService || null;
    const saveImplementation = saveService?.saveImplementation;
    const saveCubeDefaults = flavorService?.saveCurrentFaceValuesAsCubeDefaults;
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
              saveImplementation({ cubeIds: [metadata.cube_id] });
            },
          }
        : {}),
      ...(typeof saveCubeDefaults === 'function'
        ? {
            onSaveCubeDefaults: (metadata: ChromeMetadata) => {
              if (isHistoricalCubeMetadata(metadata)) {
                toast?.push?.(
                  'warn',
                  'Historical version',
                  'Spawned historical versions cannot overwrite cube defaults.',
                );
                return null;
              }
              return Reflect.apply(saveCubeDefaults, flavorService, [metadata]);
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
    this.overlayDrawHooked = false;
    this.overlayWatchdog = { timerId: null, attempts: 0 };
    this.cleanHooked = false;
    this.graphHooksWrapped = false;
    this.requestDirtyRefresh = requestDirtyRefresh;
    this.containmentService = containmentService;
    this.collisionService = collisionService;
    this.boundsReconciler = boundsReconciler;
    this.nodeMoveQueue = new Map<string, ComfyNode[]>();
    this.nodeMoveScheduled = false;
    this.reconcileScheduled = false;
    this.expandContainmentRevisionByNodeId = new Map();
    this.groupDragState = null;
  }

  swapLayout(metadata: ChromeMetadata, direction: -1 | 1): void {
    if (!this.layoutService || !metadata?.instance_id) {
      return;
    }
    const graph = this.adapter?.getApp?.()?.graph || null;
    if (!graph) {
      return;
    }
    const plan = this.resolveSwapPlan(graph, metadata.instance_id, direction);
    if (!plan) {
      return;
    }
    const { order, current, neighbor } = plan;
    const origin = this.resolveLayoutOrigin(order);
    const gaps = this.resolveLayoutGaps(order);
    this.layoutService.swapOrder({
      graph,
      aId: current.instanceId,
      bId: neighbor.instanceId,
      order,
      layout: {
        origin,
        gaps,
        minGap: 24,
      },
    });
    this.proximity?.refreshOverlayState?.({ recompute: true, graph });
    this.proximity?.schedulePreview?.({ immediate: true, verbose: true, graph });
  }

  resolveLayoutOrigin(order: readonly SwapEntry[]): Vec2 {
    let minX = Infinity;
    let minY = Infinity;
    for (const entry of order || []) {
      const bounds = entry?.bounds;
      if (!bounds) {
        continue;
      }
      minX = Math.min(minX, bounds.x);
      minY = Math.min(minY, bounds.y);
    }
    if (!Number.isFinite(minX) || !Number.isFinite(minY)) {
      return [0, 0];
    }
    return [minX, minY];
  }

  resolveLayoutGaps(order: readonly SwapEntry[]): number[] {
    const MIN_GAP = 24;
    const gaps = [];
    for (let idx = 0; idx < (order?.length || 0) - 1; idx += 1) {
      const current = order[idx];
      const next = order[idx + 1];
      const currentBounds = current?.bounds;
      const nextBounds = next?.bounds;
      if (!currentBounds || !nextBounds) {
        gaps.push(MIN_GAP);
        continue;
      }
      const spacing = nextBounds.x - (currentBounds.x + currentBounds.w);
      const resolved = Number.isFinite(spacing) ? spacing : MIN_GAP;
      gaps.push(Math.max(MIN_GAP, resolved));
    }
    return gaps;
  }

  canSwapEntry(entry: SwapEntry | null | undefined): boolean {
    const markers = entry?.markerLookup;
    if (!markers) {
      return false;
    }
    const inputs = Array.isArray(markers.inputs) ? markers.inputs : [];
    const outputs = Array.isArray(markers.outputs) ? markers.outputs : [];
    return inputs.length > 0 && outputs.length > 0;
  }

  canSwapDirection(metadata: ChromeMetadata, direction: 'left' | 'right' | -1 | 1): boolean {
    if (!metadata?.instance_id) {
      return false;
    }
    const graph = this.adapter?.getApp?.()?.graph || null;
    if (!graph) {
      return false;
    }
    const dir = direction === 'left' || direction === -1 ? -1 : 1;
    const plan = this.resolveSwapPlan(graph, metadata.instance_id, dir);
    return Boolean(plan?.neighbor);
  }

  resolveProximityMatchesForSwap(graph: ComfyGraph): ProximityMatch[] {
    if (!this.proximity?.settings?.enabled) {
      return [];
    }
    try {
      const matches = Array.isArray(this.proximity?.overlayMatches)
        ? this.proximity.overlayMatches
        : [];
      if (!graph) {
        return matches;
      }
      return matches.filter(
        (match) =>
          match?.outputNode?.graph === graph &&
          match?.inputNode?.graph === graph &&
          match?.outputId != null &&
          match?.inputId != null,
      );
    } catch (error) {
      this.adapter
        ?.getConsole?.()
        ?.warn?.('SugarCubes: failed to compute proximity swap matches', error);
      return [];
    }
  }

  resolveSwapPlan(graph: ComfyGraph, instanceId: string, direction: -1 | 1): SwapPlan | null {
    const layoutService = this.layoutService;
    if (!layoutService) {
      return null;
    }
    const index = layoutService.buildIndex(graph);
    const proximityMatches = this.resolveProximityMatchesForSwap(graph);
    const order = layoutService.deriveOrder(index, {
      graph,
      anchorInstanceId: instanceId,
      proximityMatches,
    });
    const currentIndex = order.findIndex((entry) => entry?.instanceId === instanceId);
    if (currentIndex < 0) {
      return null;
    }
    const current = order[currentIndex] as SwapEntry | undefined;
    if (!this.canSwapEntry(current)) {
      return null;
    }
    let nextIndex = currentIndex + direction;
    let neighbor = null;
    while (nextIndex >= 0 && nextIndex < order.length) {
      const candidate = order[nextIndex] as SwapEntry | undefined;
      if (this.canSwapEntry(candidate)) {
        neighbor = candidate;
        break;
      }
      nextIndex += direction;
    }
    if (!current?.instanceId || !neighbor?.instanceId) {
      return null;
    }
    return { order: order as SwapEntry[], current, neighbor };
  }

  /** Return observable chrome state without exposing mutable overlay ownership. */
  getChromeDebugState() {
    return this.chrome.getDebugState();
  }

  setup(): void {
    this.proximity.installInterceptors();
    this.chrome.setup();
    this.ensureOverlayHook();
    this.startOverlayWatchdog();
    this.ensureGraphHooks();
    this.ensureCollapseHook();
    this.ensureCleanHook();
  }

  dispose(): void {
    if (this.overlayWatchdog.timerId != null) {
      this.adapter?.getWindow?.()?.clearInterval?.(this.overlayWatchdog.timerId);
      this.overlayWatchdog.timerId = null;
    }
    this.chrome?.dispose?.();
  }

  isOverlayHookActive(canvas: OverlayCanvas | null | undefined): boolean {
    if (!canvas) {
      return false;
    }
    const liteGraph = this.adapter?.getLiteGraph?.() || null;
    const proto = (liteGraph?.LGraphCanvas?.prototype as MutablePrototype | undefined) ?? null;
    if (proto?.__sugarcubes_overlay_hooked) {
      return true;
    }
    return Boolean(
      canvas.onDrawForeground?.__sugarcubes_overlay_hooked ||
        canvas.onDrawBackground?.__sugarcubes_overlay_hooked,
    );
  }

  ensureOverlayHook(attempt = 0): void {
    const appRef = this.adapter?.getApp?.() || null;
    const canvas = (appRef?.canvas ?? appRef?.graph?.canvas ?? null) as OverlayCanvas | null;
    const MAX_RETRIES = 60;
    const RETRY_DELAY_MS = 250;
    if (!canvas) {
      if (attempt < MAX_RETRIES) {
        this.scheduler?.timeout?.(() => this.ensureOverlayHook(attempt + 1), RETRY_DELAY_MS);
      }
      return;
    }
    if (this.overlayDrawHooked && this.isOverlayHookActive(canvas)) {
      return;
    }
    const liteGraph = this.adapter?.getLiteGraph?.() || null;
    const canvasProto =
      (liteGraph?.LGraphCanvas?.prototype as MutablePrototype | undefined) ?? null;
    const manager = this;
    if (canvasProto && typeof canvasProto.drawConnections === 'function') {
      if (!canvasProto.__sugarcubes_proximity_hooked) {
        canvasProto.__sugarcubes_proximity_hooked = true;
        const originalDrawConnections = canvasProto.drawConnections;
        canvasProto.drawConnections = function drawConnections(
          this: OverlayCanvas,
          ctx: CanvasRenderingContext2D,
          ...args: unknown[]
        ) {
          const result = originalDrawConnections.call(this, ctx, ...args);
          try {
            manager.proximity.render(ctx, this);
          } catch (error) {
            manager.adapter
              ?.getConsole?.()
              ?.error?.('SugarCubes: drawConnections overlay failed', error);
          }
          return result;
        };
      }
      this.overlayDrawHooked = true;
    }
    if (canvasProto && typeof canvasProto.drawForeground === 'function') {
      if (!canvasProto.__sugarcubes_overlay_hooked) {
        canvasProto.__sugarcubes_overlay_hooked = true;
        const originalDrawForeground = canvasProto.drawForeground;
        canvasProto.drawForeground = function drawForeground(
          this: OverlayCanvas,
          ctx: CanvasRenderingContext2D,
          ...args: unknown[]
        ) {
          const result = originalDrawForeground.call(this, ctx, ...args);
          try {
            manager.placement.render(ctx, this);
            manager.chrome.render(ctx, this);
          } catch (error) {
            manager.adapter
              ?.getConsole?.()
              ?.error?.('SugarCubes: drawForeground overlay failed', error);
          }
          return result;
        };
      }
      this.overlayDrawHooked = true;
    }
    if (typeof canvas.onDrawForeground === 'function') {
      const previous = canvas.onDrawForeground;
      const wrappedForeground: HookedDrawCallback = function onDrawForeground(
        this: OverlayCanvas,
        ctx: CanvasRenderingContext2D,
        ...args: unknown[]
      ) {
        try {
          previous.call(this, ctx, ...args);
        } catch (error) {
          manager.adapter
            ?.getConsole?.()
            ?.error?.('SugarCubes: onDrawForeground wrapper failed', error);
        }
        manager.placement.render(ctx, this);
        manager.chrome.render(ctx, this);
      };
      wrappedForeground.__sugarcubes_overlay_hooked = true;
      canvas.onDrawForeground = wrappedForeground;
      this.overlayDrawHooked = true;
      if (!this.isOverlayHookActive(canvas) && attempt < MAX_RETRIES) {
        this.scheduler?.timeout?.(() => this.ensureOverlayHook(attempt + 1), RETRY_DELAY_MS);
      }
    }
    const previous = typeof canvas.onDrawBackground === 'function' ? canvas.onDrawBackground : null;
    const wrappedBackground: HookedDrawCallback = function onDrawBackground(
      this: OverlayCanvas,
      ctx: CanvasRenderingContext2D,
      ...args: unknown[]
    ) {
      try {
        previous?.call(this, ctx, ...args);
      } catch (error) {
        manager.adapter
          ?.getConsole?.()
          ?.error?.('SugarCubes: onDrawBackground wrapper failed', error);
      }
      manager.placement.render(ctx, this);
    };
    wrappedBackground.__sugarcubes_overlay_hooked = true;
    canvas.onDrawBackground = wrappedBackground;
    this.overlayDrawHooked = true;
    if (!this.isOverlayHookActive(canvas) && attempt < MAX_RETRIES) {
      this.scheduler?.timeout?.(() => this.ensureOverlayHook(attempt + 1), RETRY_DELAY_MS);
    }
  }

  startOverlayWatchdog(): void {
    if (this.overlayWatchdog.timerId != null) {
      return;
    }
    this.overlayWatchdog.attempts = 0;
    const windowRef = this.adapter?.getWindow?.() || null;
    if (!windowRef) {
      return;
    }
    this.overlayWatchdog.timerId = windowRef.setInterval(() => {
      this.overlayWatchdog.attempts += 1;
      this.ensureOverlayHook();
      const appRef = this.adapter?.getApp?.() || null;
      const canvas = (appRef?.canvas ?? appRef?.graph?.canvas ?? null) as OverlayCanvas | null;
      if (canvas && this.isOverlayHookActive(canvas)) {
        windowRef.clearInterval(this.overlayWatchdog.timerId ?? undefined);
        this.overlayWatchdog.timerId = null;
        return;
      }
      if (this.overlayWatchdog.attempts >= 20) {
        windowRef.clearInterval(this.overlayWatchdog.timerId ?? undefined);
        this.overlayWatchdog.timerId = null;
      }
    }, 500);
  }

  ensureCleanHook(attempt = 0): void {
    if (this.cleanHooked) {
      return;
    }
    const appRef = this.adapter?.getApp?.() || null;
    const MAX_RETRIES = 60;
    const RETRY_DELAY_MS = 250;
    if (typeof appRef?.clean !== 'function') {
      if (attempt < MAX_RETRIES) {
        this.scheduler?.timeout?.(() => this.ensureCleanHook(attempt + 1), RETRY_DELAY_MS);
      }
      return;
    }
    const manager = this;
    const originalClean = appRef.clean as HostCallback;
    appRef.clean = function sugarcubesPatchedClean(this: UnknownRecord, ...args: unknown[]) {
      manager.proximity?.resetOverlayState?.();
      manager.placement?.stop?.();
      return originalClean.call(this, ...args);
    };
    this.cleanHooked = true;
  }

  ensureCollapseHook(attempt = 0): void {
    const liteGraph = this.adapter?.getLiteGraph?.() || null;
    const nodeProto = (liteGraph?.LGraphNode?.prototype as MutablePrototype | undefined) ?? null;
    const MAX_RETRIES = 60;
    const RETRY_DELAY_MS = 250;
    if (!nodeProto || typeof nodeProto.collapse !== 'function') {
      if (attempt < MAX_RETRIES) {
        this.scheduler?.timeout?.(() => this.ensureCollapseHook(attempt + 1), RETRY_DELAY_MS);
      }
      return;
    }
    if (!nodeProto.__sugarcubes_collapse_hooked) {
      const originalCollapse = nodeProto.collapse;
      nodeProto.collapse = function sugarcubesPatchedCollapse(
        this: UnknownRecord,
        ...args: unknown[]
      ) {
        const wasCollapsed = isRecord(this.flags) && this.flags.collapsed === true;
        const result = originalCollapse.call(this, ...args);
        const isCollapsed = isRecord(this.flags) && this.flags.collapsed === true;
        if (wasCollapsed !== isCollapsed) {
          const collapseManager = nodeProto.__sugarcubes_collapse_manager;
          if (collapseManager instanceof OverlayManager) {
            collapseManager.onNodeCollapseToggled({
              node: this,
              wasCollapsed,
              isCollapsed,
            });
          }
        }
        return result;
      };
      nodeProto.__sugarcubes_collapse_hooked = true;
    }
    nodeProto.__sugarcubes_collapse_manager = this;
  }

  scheduleAfterFrames(callback: () => void, frames: number): void {
    const remaining = Number.isFinite(frames) ? Math.max(0, Math.floor(frames)) : 0;
    if (remaining <= 0) {
      callback();
      return;
    }
    if (typeof this.scheduler?.raf !== 'function') {
      callback();
      return;
    }
    this.scheduler.raf(() => this.scheduleAfterFrames(callback, remaining - 1));
  }

  clearGroupDragState(): void {
    this.groupDragState = null;
  }

  readManagedGroupEntries(graph: ComfyGraph | null | undefined): ManagedGroupEntry[] {
    const groups = Array.isArray(graph?._groups) ? graph._groups : graph?.groups || [];
    const entries: ManagedGroupEntry[] = [];
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

  snapshotManagedGroupBounds(graph: ComfyGraph): Map<string, RectBounds> {
    const snapshot = new Map<string, RectBounds>();
    for (const entry of this.readManagedGroupEntries(graph)) {
      snapshot.set(entry.instanceId, { ...entry.bounds });
    }
    return snapshot;
  }

  boundsMatch(a: unknown, b: RectBounds): boolean {
    if (!isRecord(a)) {
      return false;
    }
    return a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h;
  }

  commitManagedGroupBoundsChanges(
    graph: ComfyGraph,
    previous: ReadonlyMap<string, RectBounds> | null = null,
  ): CommittedGroupEntry[] {
    const entries = this.readManagedGroupEntries(graph);
    const committed: CommittedGroupEntry[] = [];
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

  resolveManagedCanvasGroup(canvas: OverlayCanvas | null | undefined): ManagedGroupTarget | null {
    const candidates: ComfyGroup[] = [];
    const addCandidate = (group: unknown): void => {
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

  resolveManagedDragFallback(): ManagedGroupTarget | null {
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
    const changedFromCapture =
      bounds[0] !== state.bounds.x ||
      bounds[1] !== state.bounds.y ||
      bounds[2] !== state.bounds.w ||
      bounds[3] !== state.bounds.h;
    if (!changedFromCapture) {
      return null;
    }
    return { group, metadata, bounds };
  }

  captureGroupDragState(canvas: OverlayCanvas): void {
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

  commitGroupDrag(
    graph: ComfyGraph,
    canvas: OverlayCanvas | null | undefined,
  ): CommittedGroupEntry | null {
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
    const changed =
      nextBounds.x !== state.bounds.x ||
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
    this.scheduleBoundsReconcile(graph);
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

  commitSelectedGroupBounds(_graph: ComfyGraph, canvas: OverlayCanvas): CommittedGroupEntry | null {
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

  ensureGraphHooks(attempt = 0): void {
    const appRef = this.adapter?.getApp?.() || null;
    const canvas = appRef?.canvas as OverlayCanvas | undefined;
    const MAX_RETRIES = 60;
    const RETRY_DELAY_MS = 250;
    if (!canvas) {
      if (attempt < MAX_RETRIES) {
        this.scheduler?.timeout?.(() => this.ensureGraphHooks(attempt + 1), RETRY_DELAY_MS);
      }
      return;
    }
    if (this.graphHooksWrapped) {
      return;
    }
    this.graphHooksWrapped = true;
    const manager = this;

    const canvasElement = (canvas.canvas as HookedCanvasElement | undefined) ?? null;
    if (canvasElement && !canvasElement.__sugarcubes_chrome_listener) {
      const handler = (event: MouseEvent | PointerEvent): boolean => {
        if (manager.chrome?.handleMouseDown?.(event, canvas)) {
          event.preventDefault();
          event.stopPropagation();
          return false;
        }
        return true;
      };
      canvasElement.addEventListener('pointerdown', handler, true);
      canvasElement.addEventListener('mousedown', handler, true);
      canvasElement.__sugarcubes_chrome_listener = handler;
    }
    if (canvasElement && !canvasElement.__sugarcubes_chrome_move_listener) {
      const moveHandler = (event: MouseEvent | PointerEvent): void => {
        if (manager.chrome?.handlePointerMove?.(event, canvas)) {
          event.preventDefault();
        }
      };
      canvasElement.addEventListener('pointermove', moveHandler, true);
      canvasElement.addEventListener('mousemove', moveHandler, true);
      canvasElement.__sugarcubes_chrome_move_listener = moveHandler;
    }

    const originalAfterChange = canvas.onAfterChange;
    canvas.onAfterChange = function onAfterChange(this: OverlayCanvas, ...args: unknown[]) {
      const managedSnapshot = manager.snapshotManagedGroupBounds(this.graph);
      const preCommit = manager.commitSelectedGroupBounds(this.graph, this);
      const result = originalAfterChange?.call(this, ...args);
      const postCommit = manager.commitSelectedGroupBounds(this.graph, this);
      const committedSet = new Map<string, CommittedGroupEntry>();
      const recordCommitted = (entry: CommittedGroupEntry | null): void => {
        const instanceId = entry?.metadata?.instance_id;
        if (!instanceId) {
          return;
        }
        committedSet.set(String(instanceId), entry);
      };
      recordCommitted(preCommit);
      recordCommitted(postCommit);
      for (const entry of manager.commitManagedGroupBoundsChanges(this.graph, managedSnapshot)) {
        recordCommitted(entry);
      }
      if (
        committedSet.size &&
        manager.containmentService &&
        manager.collisionService &&
        this.graph
      ) {
        const index = manager.containmentService.buildIndex(this.graph);
        for (const instanceId of committedSet.keys()) {
          manager.collisionService.resolveCollisions({
            graph: this.graph,
            activeInstanceId: instanceId,
            index,
          });
        }
      }
      manager.ensureOverlayHook();
      manager.enqueueNodeMove(this.graph, null, this);
      manager.flushNodeMoves(this.graph);
      manager.proximity.schedulePreview({ verbose: true, graph: this.graph });
      manager.scheduleBoundsReconcile(this.graph);
      manager.events?.emit?.('cube:instances:refresh', {
        graph: this.graph,
        reason: 'graph-change',
      });
      manager.requestDirtyRefresh?.({ graph: this.graph, reason: 'graph-change' });
      return result;
    };

    const originalNodeMoved = canvas.onNodeMoved;
    canvas.onNodeMoved = function onNodeMoved(
      this: OverlayCanvas,
      node: unknown,
      ...args: unknown[]
    ) {
      const response =
        typeof originalNodeMoved === 'function'
          ? originalNodeMoved.call(this, node, ...args)
          : undefined;
      manager.enqueueNodeMove(this.graph, node, this);
      return response;
    };

    const originalProcessMouseMove = canvas.processMouseMove;
    if (typeof originalProcessMouseMove === 'function') {
      canvas.processMouseMove = function processMouseMove(this: OverlayCanvas, ...args: unknown[]) {
        const res = originalProcessMouseMove.call(this, ...args);
        if (manager.proximity.isOverlayEnabled()) {
          manager.proximity.schedulePreview({ immediate: true, verbose: true, graph: this.graph });
        }
        return res;
      };
    }

    const originalProcessMouseDown = canvas.processMouseDown;
    if (typeof originalProcessMouseDown === 'function') {
      canvas.processMouseDown = function processMouseDown(
        this: OverlayCanvas,
        event: MouseEvent | PointerEvent,
        ...args: unknown[]
      ) {
        if (manager.chrome?.handleMouseDown?.(event, this)) {
          manager.clearGroupDragState();
          return true;
        }
        if (manager.placement.getState().active) {
          const handled = manager.placement.handlePlacementMouseDown(event, this, null);
          if (handled) {
            manager.clearGroupDragState();
            return true;
          }
        }
        const result = originalProcessMouseDown.call(this, event, ...args);
        manager.captureGroupDragState(this);
        return result;
      };
    }

    const originalProcessMouseUp = canvas.processMouseUp;
    if (typeof originalProcessMouseUp === 'function') {
      canvas.processMouseUp = function processMouseUp(this: OverlayCanvas, ...args: unknown[]) {
        const committed =
          manager.commitGroupDrag(this.graph, this) ||
          manager.commitSelectedGroupBounds(this.graph, this);
        const result = originalProcessMouseUp.call(this, ...args);
        if (committed?.group && committed?.bounds) {
          writeCanonicalBounds({
            group: committed.group,
            metadata: committed.metadata,
            bounds: committed.bounds,
          });
        }
        manager.scheduleBoundsReconcile(this.graph);
        return result;
      };
    }

    const graph = canvas.graph;
    if (graph && !graph.__sugarcubes_dirty_wrapped) {
      graph.__sugarcubes_dirty_wrapped = true;
      const wrapGraphHook = (
        key: 'onNodeAdded' | 'onNodeRemoved' | 'onNodeConnectionChange',
      ): void => {
        const original = graph[key];
        if (typeof original !== 'function') {
          return;
        }
        graph[key] = function wrappedGraphHook(this: ComfyGraph, ...args: unknown[]) {
          const result = original.call(this, ...args);
          const changedNode = isRecord(args[0]) ? args[0] : null;
          if (
            manager.proximity.isOverlayEnabled() &&
            key === 'onNodeConnectionChange' &&
            (changedNode?.type === 'SugarCubes.CubeInput' ||
              changedNode?.type === 'SugarCubes.CubeOutput')
          ) {
            manager.proximity.schedulePreview({ immediate: true, graph: this });
          }
          manager.requestDirtyRefresh?.({ graph: this, reason: key });
          return result;
        };
      };
      wrapGraphHook('onNodeAdded');
      wrapGraphHook('onNodeRemoved');
      wrapGraphHook('onNodeConnectionChange');
    }

    if (this.proximity.isOverlayEnabled()) {
      this.proximity.schedulePreview({ immediate: true, verbose: true, graph: canvas.graph });
    }

    const originalBackground = canvas.onDrawBackground;
    canvas.onDrawBackground = function onDrawBackground(
      this: OverlayCanvas,
      ctx: CanvasRenderingContext2D,
      ...args: unknown[]
    ) {
      manager.proximity.ensurePreview(this.graph);
      return originalBackground?.call(this, ctx, ...args);
    };
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

  collectMovedNodes(node: unknown, canvas: OverlayCanvas | null | undefined): ComfyNode[] {
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
    canvas: OverlayCanvas | null | undefined,
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

function isHistoricalCubeMetadata(metadata: ChromeMetadata): boolean {
  const revisionRef =
    typeof metadata?.cube_revision_ref === 'string' ? metadata.cube_revision_ref.trim() : '';
  return Boolean(revisionRef && revisionRef !== 'WORKTREE');
}
