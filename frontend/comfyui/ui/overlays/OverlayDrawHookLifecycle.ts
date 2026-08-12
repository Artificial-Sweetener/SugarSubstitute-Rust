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
 * Own installation and watchdog lifecycle for canvas overlay draw hooks.
 */

import type { PlacementOverlay } from './PlacementOverlay.js';
import type { ProximityOverlay } from './ProximityOverlay.js';
import type { CubeChromeOverlay } from './CubeChromeOverlay.js';
import type { ProximityGraphMutationTracker } from './proximity/ProximityGraphMutationTracker.js';
import type { ComfyCanvas, ComfyGraph } from '../types/graph.js';
import type { UnknownRecord } from '../types/common.js';

interface DrawHookAdapter {
  getApp?(): { canvas?: ComfyCanvas | null; graph?: ComfyGraph | null } | null;
  getWindow?(): Window | null;
  getLiteGraph?(): LiteGraphHost | null;
  getConsole?(): Console | null;
}
interface DrawHookScheduler {
  timeout?(callback: () => void, delayMs: number): number | null;
}
export interface DrawHookCanvas extends ComfyCanvas {
  graph: ComfyGraph;
  onDrawForeground?: HookedDrawCallback | null;
  onDrawBackground?: HookedDrawCallback | null;
}
interface HookedDrawCallback {
  (this: DrawHookCanvas, ctx: CanvasRenderingContext2D, ...args: unknown[]): unknown;
  __sugarcubes_overlay_hooked?: boolean;
}
interface MutablePrototype extends UnknownRecord {
  drawForeground?: (
    this: DrawHookCanvas,
    ctx: CanvasRenderingContext2D,
    ...args: unknown[]
  ) => unknown;
}

/** Maintain the active host canvas draw hook across renderer replacement. */
export class OverlayDrawHookLifecycle {
  private overlayDrawHooked = false;
  private readonly overlayWatchdog = { timerId: null as number | null, attempts: 0 };

  constructor(
    private readonly adapter: DrawHookAdapter | null,
    private readonly scheduler: DrawHookScheduler | null,
    readonly proximity: ProximityOverlay,
    readonly placement: PlacementOverlay,
    readonly chrome: CubeChromeOverlay,
    readonly proximityGraphMutations: ProximityGraphMutationTracker,
  ) {}

  dispose(): void {
    if (this.overlayWatchdog.timerId != null) {
      this.adapter?.getWindow?.()?.clearInterval?.(this.overlayWatchdog.timerId);
      this.overlayWatchdog.timerId = null;
    }
  }

  isOverlayHookActive(canvas: DrawHookCanvas | null | undefined): boolean {
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
    const canvas = (appRef?.canvas ?? appRef?.graph?.canvas ?? null) as DrawHookCanvas | null;
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
    if (canvasProto && typeof canvasProto.drawForeground === 'function') {
      if (!canvasProto.__sugarcubes_overlay_hooked) {
        canvasProto.__sugarcubes_overlay_hooked = true;
        const originalDrawForeground = canvasProto.drawForeground;
        canvasProto.drawForeground = function drawForeground(
          this: DrawHookCanvas,
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
        this: DrawHookCanvas,
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
      this: DrawHookCanvas,
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
      try {
        manager.proximityGraphMutations.attach(this.graph);
        manager.proximity.ensurePreview(this.graph);
        manager.proximity.render(ctx, this);
        manager.placement.render(ctx, this);
      } catch (error) {
        manager.adapter?.getConsole?.()?.error?.('SugarCubes: background overlays failed', error);
      }
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
      const canvas = (appRef?.canvas ?? appRef?.graph?.canvas ?? null) as DrawHookCanvas | null;
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
}
