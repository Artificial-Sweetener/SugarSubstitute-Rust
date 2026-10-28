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
/** Translate shared surface actions to the browser's camera and image owners. */

import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasActionSurface } from '../application/CanvasActions.js';
import type { CanvasCameraController } from '../application/CanvasCameraController.js';
import type { CanvasDisplayPlan } from '../application/CanvasDisplayPlan.js';
import type { CanvasCommandScope } from '../domain/CanvasViewPort.js';
import type { CanvasMediaActions } from './CanvasMediaActions.js';
import type { BrowserCanvasResources } from './BrowserCanvasResources.js';

/** Retain the existing browser owners without moving their camera or resource state. */
export class BrowserCanvasActionSurface implements CanvasActionSurface {
  /** Bind the existing per-attachment owners; their lifetimes stay with the attachment. */
  constructor(
    private readonly camera: CanvasCameraController,
    private readonly resources: BrowserCanvasResources,
    private readonly actions: CanvasMediaActions,
    private readonly openItem: (item: WorkflowOutputItem) => void,
  ) {}

  /** Preserve the browser camera's own fit transition. */
  fit(): boolean {
    return this.camera.fit();
  }

  /** Request one source pixel per physical pixel from the browser camera. */
  native(): boolean {
    return this.camera.native();
  }

  /** Apply the browser camera's existing zoom and snapping policy. */
  zoom(factor: number): boolean {
    return this.camera.zoom(factor);
  }

  /** Translate viewport fractions into the camera's current CSS-pixel geometry. */
  pan(x: number, y: number): boolean {
    const target = this.camera.read();
    return target
      ? this.camera.pan({
          x: x * target.geometry.viewport.width,
          y: y * target.geometry.viewport.height,
        })
      : false;
  }

  /** Retry the exact display plan through its resource owner. */
  retry(plan: CanvasDisplayPlan): boolean {
    this.resources.reconcile(plan, true);
    return true;
  }

  /** Ask the resource owner to admit its current large image. */
  admitLargeImage(): boolean {
    this.resources.admit();
    return true;
  }

  /** Delegate the exact selected item to the host's existing open action. */
  open(item: WorkflowOutputItem): void {
    this.openItem(item);
  }

  /** Keep copy and file actions with the existing browser transfer owner. */
  media(
    command: Parameters<CanvasActionSurface['media']>[0],
    scope: CanvasCommandScope,
  ): Promise<boolean> {
    return this.actions.run(command, scope);
  }
}
