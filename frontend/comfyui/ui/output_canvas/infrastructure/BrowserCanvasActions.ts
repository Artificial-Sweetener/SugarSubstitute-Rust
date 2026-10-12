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
/** Route validated browser intents to the existing authoritative canvas owners. */

import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasCameraController } from '../application/CanvasCameraController.js';
import type { CanvasDisplayPlan } from '../application/CanvasDisplayPlan.js';
import type { CanvasSelectionController } from '../application/CanvasSelectionController.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasCommand, CanvasViewSnapshot } from '../domain/CanvasViewPort.js';
import type { BrowserCanvasResources } from './BrowserCanvasResources.js';

export interface BrowserCanvasActionOwners {
  readonly plan: CanvasDisplayPlan;
  readonly catalog: CanvasCatalogSnapshot;
  readonly camera: CanvasCameraController;
  readonly resources: BrowserCanvasResources;
  readonly selection: CanvasSelectionController;
  readonly snapshot: CanvasViewSnapshot;
  /** Open an exact current/pinned item through the host adapter. */
  open(item: WorkflowOutputItem): void;
}

/** Dispatch one operation without adding image, navigation, or camera policy to composition. */
export function applyBrowserCanvasCommand(
  command: CanvasCommand,
  owners: BrowserCanvasActionOwners,
): boolean {
  const { plan, camera, resources, snapshot } = owners;
  switch (command.type) {
    case 'fit':
      return camera.fit();
    case 'native':
      return camera.native();
    case 'zoom':
      return camera.zoom(command.factor);
    case 'pan': {
      const target = camera.read();
      return target
        ? camera.pan({
            x: command.x * target.geometry.viewport.width,
            y: command.y * target.geometry.viewport.height,
          })
        : false;
    }
    case 'retry':
      if (plan.preview) return false;
      resources.reconcile(plan, true);
      return true;
    case 'admit-large-image':
      resources.admit();
      return true;
    case 'open-media': {
      const item = command.target === 'primary' ? plan.primary?.item : plan.secondary?.item;
      if (!item) return false;
      owners.open(item);
      return true;
    }
    case 'comparison':
      if (command.enabled && (snapshot.preview || snapshot.resource.state !== 'ready'))
        return false;
      break;
    default:
      break;
  }
  return owners.selection.apply(plan.view.viewId, owners.catalog, command);
}
