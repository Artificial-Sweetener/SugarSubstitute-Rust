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
 * Own the SugarCubes overlay rendering layer in `frontend/comfyui/ui/overlays/PlacementOverlay.js`.
 */

import { PlacementPreviewPainter } from './PlacementPreviewPainter.js';
import { PlacementPointerController } from './PlacementPointerController.js';
import { PlacementDomLifecycle } from './PlacementDomLifecycle.js';
import { PlacementCommandCoordinator } from './PlacementCommandCoordinator.js';
import type { ApiJsonResult } from '../core/CubeLibraryApi.js';
import type { PreviewEntry } from './PlacementHelpers.js';
import type { ComfyApplication, ComfyCanvas, ComfyGroup } from '../types/graph.js';
import type { UnknownRecord, Vec2 } from '../types/common.js';

export interface PlacementCanvas extends ComfyCanvas {
  canvas?: HTMLCanvasElement;
  ds?: { scale?: number; offset?: number[] };
  last_mouse_position?: number[];
  convertCanvasToOffset?(point: Vec2): unknown;
}
export interface PlacementPayload extends UnknownRecord {
  layout?: { origin?: unknown; groups?: ComfyGroup[] };
  nodes?: PreviewEntry[];
  markers?: Array<PreviewEntry & { kind?: unknown }>;
  warnings?: unknown[];
  cube?: { version?: unknown };
}
export interface PlacementResult extends UnknownRecord {
  success?: boolean;
}
export interface PlacementAdapter {
  getApp?(): ComfyApplication | null;
  getDocument?(): Document | null;
  getWindow?(): Window | null;
  getConsole?(): { warn(...values: unknown[]): void } | null;
  getLiteGraph?(): {
    vueNodesMode?: boolean;
    NODE_TITLE_HEIGHT?: number;
    NODE_COLLAPSED_WIDTH?: number;
    NODE_TEXT_SIZE?: number;
    NODE_FONT?: string;
  } | null;
  getNodeRenderer?(): 'litegraph' | 'vue';
}
export interface PlacementScheduler {
  raf?(callback: FrameRequestCallback): number | null;
  cancelRaf?(id: number): void;
}
export interface PlacementApi {
  load(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
  loadRevision(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
}
export interface PlacementBrowser {
  setBusy?(busy: boolean): void;
  close?(): void;
}
export interface PlacementToast {
  push?(severity: string, summary: string, detail?: string): void;
}
export interface PlacementStartOptions {
  defaultAlias?: string;
  revisionRef?: unknown;
  version?: string;
  origin?: Vec2;
  closeBrowser?: boolean;
}
interface PlacementHandlers {
  canvasElement: HTMLCanvasElement;
  onPointerMove: (event: PointerEvent) => void;
  onPointerDown: (event: PointerEvent) => void;
  onMouseDown: (event: MouseEvent) => void;
  onContextMenu: (event: MouseEvent) => void;
  onKeyDown: (event: KeyboardEvent) => void;
  overlayElement: HTMLDivElement;
  onOverlayPointerMove: (event: PointerEvent) => void;
  onOverlayPointerDown: (event: PointerEvent) => void;
  onOverlayPointerUp: (event: PointerEvent) => void;
  onOverlayContextMenu: (event: MouseEvent) => void;
}
export interface PlacementState extends UnknownRecord {
  active: boolean;
  cubeId: string | null;
  defaultAlias: string | null;
  payload: PlacementPayload | null;
  baseOrigin: Vec2;
  origin: Vec2;
  canvasElement: HTMLCanvasElement | null;
  handlers: PlacementHandlers | null;
  commitInProgress: boolean;
  overlayElement: HTMLDivElement | null;
  overlayRafId: number | null | undefined;
  cubeVersion: string;
  cubeRevisionRef: string;
}
export interface PlacementOverlayOptions {
  adapter?: PlacementAdapter | null;
  events?: unknown;
  scheduler?: PlacementScheduler | null;
  cubeApi?: PlacementApi | null;
  cubeBrowser?: PlacementBrowser | null;
  toast?: PlacementToast | null;
  applyPreparedImport?: (
    payload: PlacementPayload,
    options: { instanceAlias: string; dropOrigin: Vec2 },
  ) => Promise<PlacementResult | null> | PlacementResult | null;
  reportImportOutcome?: (
    defaultAlias: string,
    warnings: unknown[],
    result: PlacementResult | null,
    payload: PlacementPayload,
    options: { focus: boolean },
  ) => void;
  buildShiftedPlacementPayload?: (
    payload: PlacementPayload | null,
    shift: Vec2,
    targetOrigin: Vec2,
  ) => PlacementPayload | null;
}

/**
 * Coordinate placement overlay behavior for the SugarCubes UI.
 */
export class PlacementOverlay {
  readonly events: unknown;
  private readonly logger: { warn(...values: unknown[]): void } | null;
  private readonly state: PlacementState;
  private readonly previewPainter: PlacementPreviewPainter;
  private readonly pointer: PlacementPointerController;
  private readonly domLifecycle: PlacementDomLifecycle;
  private readonly commands: PlacementCommandCoordinator;

  constructor({
    adapter = null,
    events = null,
    scheduler = null,
    cubeApi = null,
    cubeBrowser = null,
    toast = null,
    applyPreparedImport,
    reportImportOutcome,
    buildShiftedPlacementPayload,
  }: PlacementOverlayOptions = {}) {
    this.events = events;
    this.logger = adapter?.getConsole?.() || null;
    this.state = {
      active: false,
      cubeId: null,
      defaultAlias: null,
      payload: null,
      baseOrigin: [0, 0],
      origin: [0, 0],
      canvasElement: null,
      handlers: null,
      commitInProgress: false,
      overlayElement: null,
      overlayRafId: null,
      cubeVersion: '',
      cubeRevisionRef: 'WORKTREE',
    };
    this.previewPainter = new PlacementPreviewPainter(adapter, this.state);
    this.pointer = new PlacementPointerController({
      adapter,
      state: this.state,
      logger: this.logger,
      commit: () => this.commit(),
      stop: (reason) => this.stop(reason),
    });
    this.domLifecycle = new PlacementDomLifecycle({
      adapter,
      scheduler,
      state: this.state,
      pointer: this.pointer,
      setDirty: () => this.setDirty(),
      stop: (reason) => this.stop(reason),
    });
    this.commands = new PlacementCommandCoordinator({
      adapter,
      cubeApi,
      cubeBrowser,
      toast,
      state: this.state,
      applyPreparedImport,
      reportImportOutcome,
      buildShiftedPlacementPayload,
      stop: (reason) => this.stop(reason),
      computeDropOrigin: () => this.computeDropOrigin(),
      installPlacementHandlers: () => this.installPlacementHandlers(),
      setPlacementSidebarVisibility: (active) => this.setPlacementSidebarVisibility(active),
      setDirty: () => this.setDirty(),
    });
  }

  getState(): PlacementState {
    return this.state;
  }

  setOrigin(origin: Vec2): void {
    this.pointer.setOrigin(origin);
  }

  setCommitInProgress(value: boolean): void {
    this.pointer.setCommitInProgress(value);
  }

  setDirty(): void {
    this.pointer.setDirty();
  }

  computeOriginFromEvent(event: PointerEvent | MouseEvent): Vec2 | null {
    return this.pointer.computeOriginFromEvent(event);
  }

  isPointerOverCanvas(event: PointerEvent | MouseEvent): boolean {
    return this.pointer.isPointerOverCanvas(event);
  }

  convertCanvasPoint(canvasInstance: PlacementCanvas, point: Vec2): Vec2 | null {
    return this.pointer.convertCanvasPoint(canvasInstance, point);
  }

  computeDropOrigin(): Vec2 {
    return this.pointer.computeDropOrigin();
  }

  resolvePlacementOrigin(
    canvasInstance: PlacementCanvas,
    event: PointerEvent | MouseEvent | null,
  ): Vec2 {
    return this.pointer.resolvePlacementOrigin(canvasInstance, event);
  }

  handlePlacementMouseDown(
    event: PointerEvent | MouseEvent,
    canvasInstance: PlacementCanvas,
    overlayElement: HTMLElement | null = null,
  ): boolean {
    return this.pointer.handlePlacementMouseDown(event, canvasInstance, overlayElement);
  }

  updatePlacementOverlayBounds(): void {
    this.domLifecycle.updatePlacementOverlayBounds();
  }

  stopPlacementOverlayLoop(): void {
    this.domLifecycle.stopPlacementOverlayLoop();
  }

  startPlacementOverlayLoop(): void {
    this.domLifecycle.startPlacementOverlayLoop();
  }

  ensurePlacementOverlay(): HTMLDivElement | null {
    return this.domLifecycle.ensurePlacementOverlay();
  }

  showPlacementOverlay(): void {
    this.domLifecycle.showPlacementOverlay();
  }

  hidePlacementOverlay(): void {
    this.domLifecycle.hidePlacementOverlay();
  }

  removePlacementHandlers(): void {
    this.domLifecycle.removePlacementHandlers();
  }

  stop(reason: string | null = null): void {
    this.domLifecycle.stop(reason);
  }

  setPlacementSidebarVisibility(isActive: boolean): void {
    this.domLifecycle.setPlacementSidebarVisibility(isActive);
  }

  installPlacementHandlers(): boolean {
    return this.domLifecycle.installPlacementHandlers();
  }

  async start(cubeId: string, options: PlacementStartOptions = {}): Promise<void> {
    await this.commands.start(cubeId, options);
  }

  async commit(): Promise<void> {
    await this.commands.commit();
  }

  async applyPreparedPayload(
    payload: PlacementPayload,
    options: { instanceAlias: string; dropOrigin: Vec2 },
  ): Promise<PlacementResult | null> {
    return this.commands.applyPreparedPayload(payload, options);
  }

  render(ctx: CanvasRenderingContext2D, canvasInstance: PlacementCanvas): void {
    this.previewPainter.render(ctx, canvasInstance);
  }
}
