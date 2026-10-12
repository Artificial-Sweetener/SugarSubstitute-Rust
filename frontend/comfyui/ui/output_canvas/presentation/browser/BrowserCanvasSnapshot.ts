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
/** Describe actual displayed image fidelity and loading state for reusable overlay controls. */

import type { CanvasDisplayPlan } from '../../application/CanvasDisplayPlan.js';
import { canvasComparisonAvailable } from '../../application/CanvasComparisonSelection.js';
import type { CanvasInspectionTarget } from '../../application/CanvasInspectionStore.js';
import type { CanvasCatalogSnapshot } from '../../domain/CanvasCatalog.js';
import type { CanvasAttachmentAddress } from '../../domain/CanvasIdentity.js';
import type { CanvasImageSlotSnapshot } from '../../domain/CanvasImageResource.js';
import type {
  CanvasResourceStatus,
  CanvasViewSnapshot,
  CanvasSourceSummary,
} from '../../domain/CanvasViewPort.js';

/** Project only lightweight status, keeping DOM handles out of the renderer-independent port. */
export function browserCanvasSnapshot(
  address: CanvasAttachmentAddress,
  plan: CanvasDisplayPlan,
  catalog: CanvasCatalogSnapshot,
  images: readonly CanvasImageSlotSnapshot<HTMLImageElement>[],
  camera: CanvasInspectionTarget | null,
  connected: boolean,
  previewStarted: boolean,
  sources: readonly CanvasSourceSummary[],
): CanvasViewSnapshot {
  const primary = images.find((slot) => slot.slot === 'primary');
  const secondary = images.find((slot) => slot.slot === 'secondary');
  const frame = primary?.frame;
  const preview = Boolean(frame?.preview && plan.preview);
  let resource: CanvasResourceStatus = primary?.status ?? status('empty', '');
  if (plan.primary && !plan.primary.sourcePresent)
    resource = status('unavailable', 'The selected workflow output is unavailable.');
  else if (plan.primary?.item && plan.primary.item.mediaKind !== 'image')
    resource = status('unavailable', 'Open this media in the host viewer.');
  else if (plan.preview && !previewStarted)
    resource = status(
      'loading',
      frame
        ? 'Waiting for a fresh source preview; showing the final image.'
        : 'Waiting for a fresh source preview.',
    );
  else if (plan.preview && resource.state === 'error')
    resource = {
      ...resource,
      canRetry: false,
      message: 'The source preview failed. Waiting for a fresh frame.',
    };
  else if (plan.primary && !plan.primary.item && !plan.preview)
    resource = status('unavailable', 'The requested batch image is unavailable.');
  else if (
    frame &&
    !frame.preview &&
    frame.key !== plan.primary?.item?.id &&
    resource.state === 'loading'
  )
    resource = { ...resource, message: 'Loading the selected image; showing the previous image.' };
  const physicalScale = camera ? camera.camera.scale * camera.geometry.density : 0;
  const b = secondary?.frame;
  let secondaryResource: CanvasResourceStatus | null = plan.view.comparison
    ? (secondary?.status ?? status('empty', 'Choose an output for B.'))
    : null;
  if (plan.secondary && (!plan.secondary.sourcePresent || !plan.secondary.item))
    secondaryResource = status('unavailable', 'The selected B image is unavailable.');
  else if (plan.secondary?.item && plan.secondary.item.mediaKind !== 'image')
    secondaryResource = status('unavailable', 'Open B in the host viewer.');
  if (!frame && b) resource = { ...resource, message: `${resource.message} Showing B.` };
  if (plan.view.mode === 'overview') resource = status('empty', '');
  return Object.freeze({
    schema: 1,
    address,
    revision: 0,
    view: plan.view,
    sources,
    grid: plan.grid,
    comparisonAvailable: canvasComparisonAvailable(catalog),
    selected: plan.primary?.item ?? null,
    secondary: plan.secondary?.item ?? null,
    preview,
    resource: Object.freeze(resource),
    secondaryResource: secondaryResource ? Object.freeze(secondaryResource) : null,
    runStatus: catalog.result.runStatus,
    connected,
    zoom: Object.freeze({
      mode: camera?.camera.mode ?? 'fit',
      physicalScale,
      dimensions: camera?.geometry.image ?? null,
      secondaryPhysicalScale:
        b && camera
          ? Object.freeze({
              x: (physicalScale * camera.geometry.image.width) / b.dimensions.width,
              y: (physicalScale * camera.geometry.image.height) / b.dimensions.height,
            })
          : null,
      secondaryDimensions: b?.dimensions ?? null,
    }),
  });
}

/** Describe unallocated/unsupported media without creating image resources or retry promises. */
function status(state: CanvasResourceStatus['state'], message: string): CanvasResourceStatus {
  return { state, message, canRetry: false, canAdmitLargeImage: false };
}
