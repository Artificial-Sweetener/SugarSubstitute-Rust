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
/** Route shared output controls through one selection owner and the active surface. */

import type { WorkflowOutputItem } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type {
  CanvasCommand,
  CanvasCommandScope,
  CanvasViewSnapshot,
} from '../domain/CanvasViewPort.js';
import type { CanvasDisplayPlan } from './CanvasDisplayPlan.js';
import type { CanvasSelectionController } from './CanvasSelectionController.js';
import type { CanvasSourceNavigation } from './CanvasSourceNavigation.js';

type SurfaceResult = boolean | Promise<boolean>;
type MediaCommand = Extract<CanvasCommand, { readonly type: 'copy-media' | 'file-media' }>;

/** Operations whose camera, media and resource custody belongs to the active surface. */
export interface CanvasActionSurface {
  /** Fit the current content through the surface's camera owner. */
  fit(scope: CanvasCommandScope): SurfaceResult;
  /** Show content at the surface's native pixel scale. */
  native(scope: CanvasCommandScope): SurfaceResult;
  /** Apply a zoom factor through the surface's camera owner. */
  zoom(factor: number, scope: CanvasCommandScope): SurfaceResult;
  /** Pan by fractions of the current viewport, independent of native coordinate units. */
  pan(x: number, y: number, scope: CanvasCommandScope): SurfaceResult;
  /** Retry resources for this exact display plan. */
  retry(plan: CanvasDisplayPlan, scope: CanvasCommandScope): SurfaceResult;
  /** Admit the current large image after explicit user intent. */
  admitLargeImage(scope: CanvasCommandScope): SurfaceResult;
  /** Open the exact selected output through the host's open action for this surface. */
  open(item: WorkflowOutputItem, scope: CanvasCommandScope): SurfaceResult;
  /** Apply a supported copy or file action to the selected output. */
  media(command: MediaCommand, scope: CanvasCommandScope): SurfaceResult;
}

export interface CanvasActionOwners {
  readonly plan: CanvasDisplayPlan;
  readonly catalog: CanvasCatalogSnapshot;
  readonly selection: CanvasSelectionController;
  readonly snapshot: CanvasViewSnapshot;
  readonly sources: CanvasSourceNavigation;
  readonly surface: CanvasActionSurface;
  readonly scope: CanvasCommandScope;
}

/** Keep shared source and comparison policy independent of how the surface presents pixels. */
export async function applyCanvasAction(
  command: CanvasCommand,
  owners: CanvasActionOwners,
): Promise<boolean> {
  const { plan, snapshot, surface, scope } = owners;
  if (!scope.current()) return false;
  if (command.type === 'reveal') return await owners.sources.reveal(command.target, scope);
  if (command.type === 'select-source')
    return await owners.sources.select(command.selection, scope);
  if (
    snapshot.preview &&
    (command.type === 'fit' ||
      command.type === 'native' ||
      command.type === 'zoom' ||
      command.type === 'pan')
  )
    return false;
  owners.sources.cancel();
  if (!scope.current()) return false;
  switch (command.type) {
    case 'fit':
      return surface.fit(scope);
    case 'native':
      return surface.native(scope);
    case 'zoom':
      return surface.zoom(command.factor, scope);
    case 'pan':
      return surface.pan(command.x, command.y, scope);
    case 'retry':
      return plan.preview ? false : surface.retry(plan, scope);
    case 'admit-large-image':
      return surface.admitLargeImage(scope);
    case 'open-media': {
      const item = command.target === 'primary' ? plan.primary?.item : plan.secondary?.item;
      return item ? await surface.open(item, scope) : false;
    }
    case 'copy-media':
    case 'file-media':
      return await surface.media(command, scope);
    case 'comparison':
      if (command.enabled && (snapshot.preview || snapshot.resource.state !== 'ready'))
        return false;
      break;
    default:
      break;
  }
  return owners.selection.apply(plan.view.viewId, owners.catalog, command);
}
