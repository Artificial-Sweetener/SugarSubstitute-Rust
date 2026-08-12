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
/** Own preview observation and bounded legacy-canvas redraws. */

import { CubeRendererTransitionStabilizer } from './CubeRendererTransitionStabilizer.js';
import type { ComfyRendererMode } from '../core/ComfyRendererMode.js';

export interface ComfyNodePreviewEventSource {
  addEventListener(type: 'executed', listener: EventListener): void;
  removeEventListener(type: 'executed', listener: EventListener): void;
}

interface CubeSurfacePreviewCoordinatorOptions {
  document: Document;
  rootGraph: object;
  getCurrentGraph(): object | null;
  getRendererMode(): ComfyRendererMode;
  setDirtyCanvas(foreground?: boolean, background?: boolean): void;
  refreshVuePreviews(): void;
  previewChanges?: { subscribe(listener: () => void): () => void };
  previewEvents?: ComfyNodePreviewEventSource;
}

/** Translate preview change sources into renderer-specific refresh behavior. */
export class CubeSurfacePreviewCoordinator {
  private readonly stabilizer: CubeRendererTransitionStabilizer;
  private readonly unsubscribe: () => void;
  private readonly onExecuted: EventListener;

  constructor(private readonly options: CubeSurfacePreviewCoordinatorOptions) {
    const windowRef = options.document.defaultView;
    this.stabilizer = new CubeRendererTransitionStabilizer({
      requestFrame: (callback) => windowRef?.requestAnimationFrame(callback) ?? null,
      cancelFrame: (handle) => windowRef?.cancelAnimationFrame(handle),
      frameCount: 120,
    });
    this.unsubscribe =
      options.previewChanges?.subscribe(() => options.refreshVuePreviews()) ?? (() => undefined);
    this.onExecuted = () => {
      options.refreshVuePreviews();
      if (options.getRendererMode() === 'litegraph') this.scheduleLegacyRedraw();
    };
    options.previewEvents?.addEventListener('executed', this.onExecuted);
  }

  /** Repaint while Comfy decodes native legacy preview images. */
  scheduleLegacyRedraw(): void {
    const redraw = (): void => {
      if (
        this.options.getRendererMode() !== 'litegraph' ||
        this.options.getCurrentGraph() !== this.options.rootGraph
      ) {
        return;
      }
      this.options.setDirtyCanvas(true, true);
    };
    redraw();
    this.stabilizer.run(redraw);
  }

  /** Remove every preview subscription and delayed redraw. */
  dispose(): void {
    this.options.previewEvents?.removeEventListener('executed', this.onExecuted);
    this.unsubscribe();
    this.stabilizer.dispose();
  }
}
