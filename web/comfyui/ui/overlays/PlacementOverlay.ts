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
 * Own the SugarCubes overlay rendering layer in `web/comfyui/ui/overlays/PlacementOverlay.js`.
 */

import { getGroupSugarcubes } from '../graph/GroupMetadata.js';
import { readVector2 } from '../graph/VectorUtils.js';
import {
  computePayloadBounds,
  drawGhostRect,
  getPlacementGroupLabel,
  resolvePreviewRect,
} from './PlacementHelpers.js';
import { isCurrentRevisionRef, normalizeRevisionRef } from '../core/CubeDefinitionKey.js';
import { isRecord } from '../types/common.js';
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
interface PlacementResult extends UnknownRecord {
  success?: boolean;
}
export interface PlacementAdapter {
  getApp?(): ComfyApplication | null;
  getDocument?(): Document | null;
  getWindow?(): Window | null;
  getConsole?(): { warn(...values: unknown[]): void } | null;
  getLiteGraph?(): {
    NODE_TITLE_HEIGHT?: number;
    NODE_COLLAPSED_WIDTH?: number;
    NODE_TEXT_SIZE?: number;
    NODE_FONT?: string;
  } | null;
}
interface PlacementScheduler {
  raf?(callback: FrameRequestCallback): number | null;
  cancelRaf?(id: number): void;
}
export interface PlacementApi {
  load(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
  loadRevision(payload: BodyInit | null, options?: RequestInit): Promise<ApiJsonResult>;
}
interface PlacementBrowser {
  setBusy?(busy: boolean): void;
  close?(): void;
}
interface PlacementToast {
  push?(severity: string, summary: string, detail?: string): void;
}
interface PlacementStartOptions {
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

const PLACEMENT_GROUP_FILL = 'rgba(70, 120, 150, 0.12)';
const PLACEMENT_GROUP_STROKE = 'rgba(70, 120, 150, 0.55)';
const PLACEMENT_NODE_FILL = 'rgba(120, 180, 210, 0.16)';
const PLACEMENT_NODE_STROKE = 'rgba(120, 180, 210, 0.6)';
const PLACEMENT_INPUT_FILL = 'rgba(90, 200, 120, 0.2)';
const PLACEMENT_INPUT_STROKE = 'rgba(90, 200, 120, 0.7)';
const PLACEMENT_OUTPUT_FILL = 'rgba(90, 140, 240, 0.2)';
const PLACEMENT_OUTPUT_STROKE = 'rgba(90, 140, 240, 0.7)';

/**
 * Coordinate placement overlay behavior for the SugarCubes UI.
 */
export class PlacementOverlay {
  private readonly adapter: PlacementAdapter | null;
  readonly events: unknown;
  private readonly scheduler: PlacementScheduler | null;
  private readonly cubeApi: PlacementApi | null;
  private readonly cubeBrowser: PlacementBrowser | null;
  private readonly toast: PlacementToast | null;
  private readonly applyPreparedImport: PlacementOverlayOptions['applyPreparedImport'];
  private readonly reportImportOutcome: PlacementOverlayOptions['reportImportOutcome'];
  private readonly buildShiftedPlacementPayload: PlacementOverlayOptions['buildShiftedPlacementPayload'];
  private readonly logger: { warn(...values: unknown[]): void } | null;
  private readonly state: PlacementState;

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
    this.adapter = adapter;
    this.events = events;
    this.scheduler = scheduler;
    this.cubeApi = cubeApi;
    this.cubeBrowser = cubeBrowser;
    this.toast = toast;
    this.applyPreparedImport = applyPreparedImport;
    this.reportImportOutcome = reportImportOutcome;
    this.buildShiftedPlacementPayload = buildShiftedPlacementPayload;
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
  }

  private getCanvas(): PlacementCanvas | null {
    const canvas = this.adapter?.getApp?.()?.canvas ?? this.adapter?.getApp?.()?.graph?.canvas;
    return isRecord(canvas) ? (canvas as PlacementCanvas) : null;
  }

  getState(): PlacementState {
    return this.state;
  }

  setOrigin(origin: Vec2): void {
    this.state.origin = origin;
  }

  setCommitInProgress(value: boolean): void {
    this.state.commitInProgress = Boolean(value);
  }

  setDirty(): void {
    this.adapter?.getApp?.()?.canvas?.setDirty?.(true, true);
  }

  computeOriginFromEvent(event: PointerEvent | MouseEvent): Vec2 | null {
    const canvasInstance = this.getCanvas();
    const canvasElement = canvasInstance?.canvas ?? null;
    if (!canvasInstance || !canvasElement) {
      return null;
    }
    if (typeof canvasElement.getBoundingClientRect !== 'function') {
      return null;
    }
    const rect = canvasElement.getBoundingClientRect();
    const relative: Vec2 = [event.clientX - rect.left, event.clientY - rect.top];
    return this.convertCanvasPoint(canvasInstance, relative);
  }

  isPointerOverCanvas(event: PointerEvent | MouseEvent): boolean {
    const canvasInstance = this.getCanvas();
    const canvasElement = canvasInstance?.canvas ?? null;
    if (!canvasElement || typeof canvasElement.getBoundingClientRect !== 'function') {
      return false;
    }
    const rect = canvasElement.getBoundingClientRect();
    return (
      event.clientX >= rect.left &&
      event.clientX <= rect.right &&
      event.clientY >= rect.top &&
      event.clientY <= rect.bottom
    );
  }

  convertCanvasPoint(canvasInstance: PlacementCanvas, point: Vec2): Vec2 | null {
    if (!canvasInstance || !Array.isArray(point)) {
      return null;
    }
    try {
      if (typeof canvasInstance.convertCanvasToOffset === 'function') {
        const converted = canvasInstance.convertCanvasToOffset(point);
        if (Array.isArray(converted) && converted.length >= 2) {
          const x = Number(converted[0]);
          const y = Number(converted[1]);
          if (Number.isFinite(x) && Number.isFinite(y)) {
            return [x, y];
          }
        }
      }
      const ds = canvasInstance.ds;
      const scale = Number(ds?.scale) || 1;
      const offset = Array.isArray(ds?.offset) ? ds.offset : [0, 0];
      const x = point[0] / scale - (offset[0] ?? 0);
      const y = point[1] / scale - (offset[1] ?? 0);
      if (Number.isFinite(x) && Number.isFinite(y)) {
        return [x, y];
      }
    } catch (error) {
      this.logger?.warn?.('SugarCubes -> convertCanvasPoint failed', error);
    }
    return null;
  }

  computeDropOrigin(): Vec2 {
    const canvasInstance = this.getCanvas();
    if (!canvasInstance) {
      return [0, 0];
    }

    const lastMouse = canvasInstance.last_mouse_position;
    if (
      Array.isArray(lastMouse) &&
      Number.isFinite(lastMouse[0]) &&
      Number.isFinite(lastMouse[1])
    ) {
      const converted = this.convertCanvasPoint(canvasInstance, [
        Number(lastMouse[0]),
        Number(lastMouse[1]),
      ]);
      if (converted) {
        return converted;
      }
    }

    try {
      const canvasElement = canvasInstance.canvas ?? null;
      if (canvasElement && typeof canvasElement.getBoundingClientRect === 'function') {
        const rect = canvasElement.getBoundingClientRect();
        const relative: Vec2 = [rect.width / 2, rect.height / 2];
        const converted = this.convertCanvasPoint(canvasInstance, relative);
        if (converted) {
          return converted;
        }
      }
    } catch (_error) {
      // ignore viewport conversion issues
    }

    const ds = canvasInstance.ds ?? null;
    if (ds) {
      const offset = Array.isArray(ds.offset) ? ds.offset : [0, 0];
      const x = -Number(offset[0] ?? 0);
      const y = -Number(offset[1] ?? 0);
      if (Number.isFinite(x) && Number.isFinite(y)) {
        return [x, y];
      }
    }

    return [0, 0];
  }

  resolvePlacementOrigin(
    canvasInstance: PlacementCanvas,
    event: PointerEvent | MouseEvent | null,
  ): Vec2 {
    if (event && typeof event.clientX === 'number' && typeof event.clientY === 'number') {
      const fromEvent = this.computeOriginFromEvent(event);
      if (fromEvent) {
        return fromEvent;
      }
    }
    const lastMouse = canvasInstance?.last_mouse_position;
    if (
      Array.isArray(lastMouse) &&
      Number.isFinite(lastMouse[0]) &&
      Number.isFinite(lastMouse[1])
    ) {
      const converted = this.convertCanvasPoint(canvasInstance, [
        Number(lastMouse[0]),
        Number(lastMouse[1]),
      ]);
      if (converted) {
        return converted;
      }
    }
    return Array.isArray(this.state.origin) ? this.state.origin : this.computeDropOrigin();
  }

  handlePlacementMouseDown(
    event: PointerEvent | MouseEvent,
    canvasInstance: PlacementCanvas,
    overlayElement: HTMLElement | null = null,
  ): boolean {
    if (!this.state.active || this.state.commitInProgress) {
      return false;
    }
    const button = typeof event?.button === 'number' ? event.button : 0;
    if (button === 0) {
      event?.preventDefault?.();
      event?.stopPropagation?.();
      event?.stopImmediatePropagation?.();
      if (event) {
        event.cancelBubble = true;
      }
      const pointerId = 'pointerId' in event ? event.pointerId : null;
      if (overlayElement?.setPointerCapture && pointerId != null) {
        try {
          overlayElement.setPointerCapture(pointerId);
        } catch (_error) {
          // ignore pointer capture failures
        }
      }
      this.state.origin = this.resolvePlacementOrigin(canvasInstance, event);
      this.state.commitInProgress = true;
      void this.commit().finally(() => {
        this.state.commitInProgress = false;
      });
      return true;
    }
    if (button === 2) {
      event?.preventDefault?.();
      event?.stopPropagation?.();
      event?.stopImmediatePropagation?.();
      if (event) {
        event.cancelBubble = true;
      }
      this.stop('Placement cancelled.');
      return true;
    }
    return false;
  }

  updatePlacementOverlayBounds(): void {
    const overlay = this.state.overlayElement;
    const canvasElement = this.state.canvasElement;
    if (!overlay || !canvasElement || typeof canvasElement.getBoundingClientRect !== 'function') {
      return;
    }
    const rect = canvasElement.getBoundingClientRect();
    overlay.style.left = `${rect.left}px`;
    overlay.style.top = `${rect.top}px`;
    overlay.style.width = `${rect.width}px`;
    overlay.style.height = `${rect.height}px`;
  }

  stopPlacementOverlayLoop(): void {
    if (this.state.overlayRafId != null) {
      this.scheduler?.cancelRaf?.(this.state.overlayRafId);
      this.state.overlayRafId = null;
    }
  }

  startPlacementOverlayLoop(): void {
    this.stopPlacementOverlayLoop();
    const tick: FrameRequestCallback = () => {
      if (!this.state.active || !this.state.overlayElement) {
        this.state.overlayRafId = null;
        return;
      }
      this.updatePlacementOverlayBounds();
      this.state.overlayRafId = this.scheduler?.raf?.(tick);
    };
    this.state.overlayRafId = this.scheduler?.raf?.(tick);
  }

  ensurePlacementOverlay(): HTMLDivElement | null {
    if (this.state.overlayElement) {
      return this.state.overlayElement;
    }
    const documentRef = this.adapter?.getDocument?.();
    if (!documentRef?.body) {
      return null;
    }
    const overlay = documentRef.createElement('div');
    overlay.className = 'sugarcubes-placement-overlay';
    Object.assign(overlay.style, {
      position: 'fixed',
      zIndex: '9005',
      background: 'transparent',
      pointerEvents: 'none',
      left: '0px',
      top: '0px',
      width: '0px',
      height: '0px',
    });
    documentRef.body.appendChild(overlay);
    this.state.overlayElement = overlay;
    return overlay;
  }

  showPlacementOverlay(): void {
    const overlay = this.ensurePlacementOverlay();
    if (!overlay) {
      return;
    }
    overlay.style.pointerEvents = 'auto';
    overlay.style.display = 'block';
    this.updatePlacementOverlayBounds();
    this.startPlacementOverlayLoop();
  }

  hidePlacementOverlay(): void {
    const overlay = this.state.overlayElement;
    if (!overlay) {
      return;
    }
    overlay.style.pointerEvents = 'none';
    overlay.style.display = 'none';
    this.stopPlacementOverlayLoop();
  }

  removePlacementHandlers(): void {
    if (!this.state.handlers) {
      return;
    }
    const {
      canvasElement,
      onPointerMove,
      onPointerDown,
      onMouseDown,
      onContextMenu,
      onKeyDown,
      overlayElement,
      onOverlayPointerMove,
      onOverlayPointerDown,
      onOverlayPointerUp,
      onOverlayContextMenu,
    } = this.state.handlers;
    if (canvasElement) {
      canvasElement.removeEventListener('pointermove', onPointerMove);
      canvasElement.removeEventListener('pointerdown', onPointerDown);
      canvasElement.removeEventListener('mousedown', onMouseDown);
      canvasElement.removeEventListener('contextmenu', onContextMenu);
    }
    if (overlayElement) {
      overlayElement.removeEventListener('pointermove', onOverlayPointerMove);
      overlayElement.removeEventListener('pointerdown', onOverlayPointerDown);
      overlayElement.removeEventListener('pointerup', onOverlayPointerUp);
      overlayElement.removeEventListener('contextmenu', onOverlayContextMenu);
    }
    this.adapter?.getWindow?.()?.removeEventListener?.('keydown', onKeyDown);
    this.state.handlers = null;
    this.state.canvasElement = null;
  }

  stop(_reason: string | null = null): void {
    if (!this.state.active) {
      return;
    }
    this.removePlacementHandlers();
    this.hidePlacementOverlay();
    this.state.active = false;
    this.state.cubeId = null;
    this.state.defaultAlias = null;
    this.state.payload = null;
    this.state.baseOrigin = [0, 0];
    this.state.origin = [0, 0];
    this.state.commitInProgress = false;
    this.state.cubeVersion = '';
    this.state.cubeRevisionRef = 'WORKTREE';
    this.setPlacementSidebarVisibility(false);
    this.setDirty();
  }

  setPlacementSidebarVisibility(isActive: boolean): void {
    const documentRef = this.adapter?.getDocument?.();
    if (!documentRef?.body) {
      return;
    }
    const active = Boolean(isActive);
    documentRef.body.classList.toggle('sugarcubes-placement--active', active);
    const panel =
      this.adapter?.getDocument?.()?.querySelector<HTMLElement>('.side-bar-panel') ||
      documentRef.querySelector<HTMLElement>('.side-bar-panel');
    if (!panel) {
      return;
    }
    if (active) {
      if (panel.dataset.sugarcubesDisplay === undefined) {
        panel.dataset.sugarcubesDisplay = panel.style.display || '';
      }
      panel.style.display = 'none';
      return;
    }
    if (panel.dataset.sugarcubesDisplay !== undefined) {
      panel.style.display = panel.dataset.sugarcubesDisplay;
      delete panel.dataset.sugarcubesDisplay;
    } else {
      panel.style.display = '';
    }
  }

  installPlacementHandlers(): boolean {
    const canvasInstance = this.getCanvas();
    const canvasElement = canvasInstance?.canvas ?? null;
    if (!canvasInstance || !canvasElement) {
      return false;
    }
    const overlayElement = this.ensurePlacementOverlay();
    if (!overlayElement) {
      return false;
    }
    const onPointerMove = (event: PointerEvent) => {
      if (!this.state.active) {
        return;
      }
      const origin = this.resolvePlacementOrigin(canvasInstance, event);
      if (!origin) {
        return;
      }
      this.state.origin = origin;
      this.setDirty();
    };
    const onPointerDown = (event: PointerEvent) => {
      this.handlePlacementMouseDown(event, canvasInstance, overlayElement);
    };
    const onMouseDown = (event: MouseEvent) => {
      this.handlePlacementMouseDown(event, canvasInstance, overlayElement);
    };
    const onContextMenu = (event: MouseEvent) => {
      if (!this.state.active) {
        return;
      }
      event.preventDefault();
    };
    const onOverlayPointerMove = (event: PointerEvent) => {
      if (!this.state.active) {
        return;
      }
      const origin = this.resolvePlacementOrigin(canvasInstance, event);
      if (!origin) {
        return;
      }
      this.state.origin = origin;
      this.setDirty();
    };
    const onOverlayPointerDown = (event: PointerEvent) => {
      this.handlePlacementMouseDown(event, canvasInstance, overlayElement);
    };
    const onOverlayPointerUp = (event: PointerEvent) => {
      if (!this.state.active) {
        return;
      }
      event?.preventDefault?.();
      event?.stopPropagation?.();
      event?.stopImmediatePropagation?.();
      if (event) {
        event.cancelBubble = true;
      }
      if (overlayElement?.releasePointerCapture && event?.pointerId != null) {
        try {
          overlayElement.releasePointerCapture(event.pointerId);
        } catch (_error) {
          // ignore pointer capture failures
        }
      }
    };
    const onOverlayContextMenu = (event: MouseEvent) => {
      if (!this.state.active) {
        return;
      }
      event.preventDefault();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (!this.state.active) {
        return;
      }
      if (event.key === 'Escape') {
        event.preventDefault();
        this.stop('Placement cancelled.');
      }
    };
    canvasElement.addEventListener('pointermove', onPointerMove);
    canvasElement.addEventListener('pointerdown', onPointerDown);
    canvasElement.addEventListener('mousedown', onMouseDown);
    canvasElement.addEventListener('contextmenu', onContextMenu);
    overlayElement.addEventListener('pointermove', onOverlayPointerMove);
    overlayElement.addEventListener('pointerdown', onOverlayPointerDown);
    overlayElement.addEventListener('pointerup', onOverlayPointerUp);
    overlayElement.addEventListener('contextmenu', onOverlayContextMenu);
    this.adapter?.getWindow?.()?.addEventListener?.('keydown', onKeyDown);
    this.state.handlers = {
      canvasElement,
      onPointerMove,
      onPointerDown,
      onMouseDown,
      onContextMenu,
      onKeyDown,
      overlayElement,
      onOverlayPointerMove,
      onOverlayPointerDown,
      onOverlayPointerUp,
      onOverlayContextMenu,
    };
    this.state.canvasElement = canvasElement;
    this.showPlacementOverlay();
    return true;
  }

  async start(cubeId: string, options: PlacementStartOptions = {}): Promise<void> {
    const trimmed = typeof cubeId === 'string' ? cubeId.trim() : '';
    if (!trimmed) {
      this.toast?.push?.('warn', 'Cube required', 'Select a cube before placing.');
      return;
    }
    const displayName =
      typeof options.defaultAlias === 'string' && options.defaultAlias.trim()
        ? options.defaultAlias.trim()
        : trimmed;
    if (!this.cubeApi) {
      this.toast?.push?.('error', 'Placement unavailable', 'Cube library API is unavailable.');
      return;
    }
    this.stop();
    this.cubeBrowser?.setBusy?.(true);
    try {
      const revisionRef = normalizeRevisionRef(options.revisionRef);
      const loader = isCurrentRevisionRef(revisionRef)
        ? this.cubeApi.load.bind(this.cubeApi)
        : this.cubeApi.loadRevision.bind(this.cubeApi);
      const body = isCurrentRevisionRef(revisionRef)
        ? { cube_id: trimmed, origin: { x: 0, y: 0 } }
        : { cube_id: trimmed, revision_ref: revisionRef, origin: { x: 0, y: 0 } };
      const { response, data } = await loader(JSON.stringify(body), {
        headers: { 'Content-Type': 'application/json' },
      });
      if (!response.ok || data?.error) {
        const errorPayload = isRecord(data.error) ? data.error : {};
        const message =
          typeof errorPayload.message === 'string'
            ? errorPayload.message
            : response.statusText || 'Placement preview failed';
        const detail =
          typeof errorPayload.detail === 'string' && errorPayload.detail ? errorPayload.detail : '';
        this.toast?.push?.('error', message, detail);
        return;
      }

      this.state.active = true;
      this.state.cubeId = trimmed;
      this.state.defaultAlias = displayName;
      const placementPayload = data as PlacementPayload;
      this.state.payload = placementPayload;
      this.state.cubeVersion =
        typeof options.version === 'string' && options.version.trim()
          ? options.version.trim()
          : typeof placementPayload.cube?.version === 'string'
            ? placementPayload.cube.version.trim()
            : '';
      this.state.cubeRevisionRef = revisionRef;
      this.state.baseOrigin = readVector2(placementPayload.layout?.origin, 0, 0);
      this.state.origin = Array.isArray(options.origin) ? options.origin : this.computeDropOrigin();

      const handlersInstalled = this.installPlacementHandlers();
      if (!handlersInstalled) {
        this.stop('Placement unavailable: canvas missing.');
        return;
      }
      this.setPlacementSidebarVisibility(true);

      if (options.closeBrowser) {
        this.cubeBrowser?.close?.();
        this.toast?.push?.(
          'info',
          'Place SugarCube',
          'Click on the canvas to place it. Press Esc to cancel.',
        );
      }

      this.setDirty();
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : String(error);
      this.toast?.push?.('error', 'Placement failed', message);
    } finally {
      this.cubeBrowser?.setBusy?.(false);
    }
  }

  async commit(): Promise<void> {
    if (!this.state.active || !this.state.cubeId) {
      return;
    }
    const defaultAlias = this.state.defaultAlias || this.state.cubeId;
    const baseOrigin: Vec2 = this.state.baseOrigin;
    const targetOrigin: Vec2 = this.state.origin;
    const shift: Vec2 = [targetOrigin[0] - baseOrigin[0], targetOrigin[1] - baseOrigin[1]];
    const payload = this.buildShiftedPlacementPayload?.(this.state.payload, shift, targetOrigin);
    this.stop();
    if (!payload) {
      return;
    }
    this.cubeBrowser?.setBusy?.(true);
    const result = await this.applyPreparedPayload(payload, {
      instanceAlias: defaultAlias,
      dropOrigin: targetOrigin,
    });
    const backendWarnings = Array.isArray(payload?.warnings)
      ? payload.warnings.filter(Boolean)
      : [];
    this.reportImportOutcome?.(defaultAlias, backendWarnings, result, payload, { focus: false });
    this.cubeBrowser?.setBusy?.(false);
    if (result?.success) {
      this.cubeBrowser?.close?.();
    }
  }

  /** Apply a prepared import through the configured application boundary. */
  async applyPreparedPayload(
    payload: PlacementPayload,
    options: { instanceAlias: string; dropOrigin: Vec2 },
  ): Promise<PlacementResult | null> {
    return (await this.applyPreparedImport?.(payload, options)) ?? null;
  }

  render(ctx: CanvasRenderingContext2D, canvasInstance: PlacementCanvas): void {
    if (!this.state.active || !this.state.payload) {
      return;
    }
    if (!canvasInstance || canvasInstance.graph !== this.adapter?.getApp?.()?.graph) {
      return;
    }

    const payload = this.state.payload;
    const baseOrigin = readVector2(payload?.layout?.origin, 0, 0);
    const currentOrigin = Array.isArray(this.state.origin) ? this.state.origin : baseOrigin;
    const shiftX = currentOrigin[0] - baseOrigin[0];
    const shiftY = currentOrigin[1] - baseOrigin[1];
    const scale = Number(canvasInstance?.ds?.scale) || 1;
    const liteGraph = this.adapter?.getLiteGraph?.() || null;

    const nodes = Array.isArray(payload?.nodes) ? payload.nodes : [];
    const markers = Array.isArray(payload?.markers) ? payload.markers : [];
    const entries = nodes.concat(markers);

    const groups = Array.isArray(payload?.layout?.groups) ? payload.layout.groups : [];
    if (groups.length) {
      for (const group of groups) {
        if (!group || typeof group !== 'object') {
          continue;
        }
        const bounding = Array.isArray(group.bounding) ? group.bounding : null;
        if (!bounding || bounding.length !== 4) {
          continue;
        }
        const [bx = 0, by = 0, bw = 0, bh = 0] = bounding.map((value) => Number(value) || 0);
        const rect = {
          x: baseOrigin[0] + bx + shiftX,
          y: baseOrigin[1] + by + shiftY,
          w: bw,
          h: bh,
        };
        drawGhostRect(
          ctx,
          rect,
          { fill: PLACEMENT_GROUP_FILL, stroke: PLACEMENT_GROUP_STROKE, alpha: 0.8 },
          scale,
          getPlacementGroupLabel(this.state.defaultAlias, group, getGroupSugarcubes),
        );
      }
    } else {
      const bounds = computePayloadBounds(entries, ctx, liteGraph);
      if (bounds) {
        drawGhostRect(
          ctx,
          {
            x: bounds.minX + shiftX,
            y: bounds.minY + shiftY,
            w: bounds.maxX - bounds.minX,
            h: bounds.maxY - bounds.minY,
          },
          { fill: PLACEMENT_GROUP_FILL, stroke: PLACEMENT_GROUP_STROKE, alpha: 0.6 },
          scale,
          this.state.defaultAlias,
        );
      }
    }

    for (const entry of nodes) {
      const layout = entry?.layout;
      if (!Array.isArray(layout?.pos) || !Array.isArray(layout?.size)) {
        continue;
      }
      const pos = readVector2(layout.pos, 0, 0);
      const size = readVector2(layout.size, 0, 0);
      const rect = resolvePreviewRect(entry, pos, size, ctx, liteGraph);
      drawGhostRect(
        ctx,
        {
          x: rect.x + shiftX,
          y: rect.y + shiftY,
          w: rect.w,
          h: rect.h,
        },
        { fill: PLACEMENT_NODE_FILL, stroke: PLACEMENT_NODE_STROKE, alpha: 0.9 },
        scale,
      );
    }

    for (const entry of markers) {
      const layout = entry?.layout;
      if (!Array.isArray(layout?.pos) || !Array.isArray(layout?.size)) {
        continue;
      }
      const pos = readVector2(layout.pos, 0, 0);
      const size = readVector2(layout.size, 0, 0);
      const rect = resolvePreviewRect(entry, pos, size, ctx, liteGraph);
      const kind = typeof entry?.kind === 'string' ? entry.kind : '';
      let style = { fill: PLACEMENT_NODE_FILL, stroke: PLACEMENT_NODE_STROKE, alpha: 0.9 };
      if (kind === 'input') {
        style = { fill: PLACEMENT_INPUT_FILL, stroke: PLACEMENT_INPUT_STROKE, alpha: 0.95 };
      } else if (kind === 'output') {
        style = { fill: PLACEMENT_OUTPUT_FILL, stroke: PLACEMENT_OUTPUT_STROKE, alpha: 0.95 };
      }
      drawGhostRect(
        ctx,
        {
          x: rect.x + shiftX,
          y: rect.y + shiftY,
          w: rect.w + 0,
          h: rect.h + 0,
        },
        style,
        scale,
      );
    }
  }
}
