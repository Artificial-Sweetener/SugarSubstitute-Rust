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
/** Offer "Change canvas resolution" on the width/height fields that size a mask-only canvas. */
import type { ContextMenuEntry } from '../../menus/ContextMenu.js';
import type { WorkflowCanvasResolution } from '../application/WorkflowCanvasResolution.js';
import { WORKFLOW_SURFACE_COPY } from '../application/WorkflowSurfaceCopy.js';
import type { CanvasResolutionDialog } from './CanvasResolutionDialog.js';
import type { ProvisionedSurface } from '../application/WorkflowSurfaceProvisioning.js';

/** One native field addressed by its flattened workflow node path and input name. */
export interface CanvasResolutionFieldTarget {
  readonly nodeId: string;
  readonly inputName: string;
}

/** A node context-menu item in the host's contribution format. */
export interface CanvasResolutionNodeMenuItem {
  readonly content: string;
  callback(): void;
}

/** Contribute the entry only when a targeted field is an authority of a provisioned surface. */
export function canvasResolutionEntries(
  targets: readonly CanvasResolutionFieldTarget[],
  resolution: Pick<WorkflowCanvasResolution, 'surface' | 'resize'>,
  dialog: Pick<CanvasResolutionDialog, 'open'>,
  report: (error: unknown) => void,
): ContextMenuEntry[] {
  const surface =
    targets
      .map((target) => resolution.surface(target.nodeId, target.inputName))
      .find((item) => item !== null) ?? null;
  if (!surface) return [];
  return [
    { label: '', separator: true },
    {
      label: WORKFLOW_SURFACE_COPY.changeCanvasResolutionMenu,
      select: () => openResolution(surface, resolution, dialog, report),
    },
  ];
}

/** Contribute the entry to the node context menu of a node that sizes a mask-only surface. */
export function canvasResolutionNodeMenuItems(
  node: string | null,
  resolution: Pick<WorkflowCanvasResolution, 'surfaceSizedBy' | 'resize'>,
  dialog: Pick<CanvasResolutionDialog, 'open'>,
  report: (error: unknown) => void,
): CanvasResolutionNodeMenuItem[] {
  const surface = node === null ? null : resolution.surfaceSizedBy(node);
  if (!surface) return [];
  return [
    {
      content: WORKFLOW_SURFACE_COPY.changeCanvasResolutionMenu,
      callback: () => openResolution(surface, resolution, dialog, report),
    },
  ];
}

/** Open the dialog at the surface's current size and resize it on apply. */
function openResolution(
  surface: ProvisionedSurface,
  resolution: Pick<WorkflowCanvasResolution, 'resize'>,
  dialog: Pick<CanvasResolutionDialog, 'open'>,
  report: (error: unknown) => void,
): void {
  dialog
    .open(
      { width: surface.document.width, height: surface.document.height },
      (width, height, mode) => resolution.resize(surface.key, width, height, mode),
    )
    .catch(report);
}
