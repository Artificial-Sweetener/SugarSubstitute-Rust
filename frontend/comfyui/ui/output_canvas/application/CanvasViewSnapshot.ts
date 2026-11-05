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
/** Assemble shared output controls from renderer-owned presentation status. */

import type { ImageDimensions } from '../../execution/domain/WorkflowOutput.js';
import type { OutputMediaProfile } from '../../output_saving/domain/OutputMedia.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasAttachmentAddress } from '../domain/CanvasIdentity.js';
import type {
  CanvasResourceStatus,
  CanvasSourceSummary,
  CanvasViewSnapshot,
  CanvasZoomStatus,
} from '../domain/CanvasViewPort.js';
import type { CanvasDisplayPlan } from './CanvasDisplayPlan.js';

/** Project presenter-specific facts without borrowing its image handles. */
export interface CanvasSnapshotPresentation {
  readonly comparisonAvailable: boolean;
  readonly preview: boolean;
  readonly resource: CanvasResourceStatus;
  readonly secondaryResource: CanvasResourceStatus | null;
  readonly zoom: CanvasZoomStatus | null;
}

/** Preserve the same workflow and selection authority for every presentation backend. */
export function canvasViewSnapshot(
  address: CanvasAttachmentAddress,
  plan: CanvasDisplayPlan,
  catalog: CanvasCatalogSnapshot,
  sources: readonly CanvasSourceSummary[],
  connected: boolean,
  mediaActions: OutputMediaProfile | null,
  presentation: CanvasSnapshotPresentation,
): CanvasViewSnapshot {
  return Object.freeze({
    schema: 1,
    address,
    revision: 0,
    view: plan.view,
    sources: catalog.sceneOverview ? Object.freeze([]) : sources,
    scenes: catalog.scenes ?? Object.freeze([]),
    grid: plan.grid,
    comparisonAvailable: presentation.comparisonAvailable,
    selected: plan.primary?.item ?? null,
    secondary: plan.secondary?.item ?? null,
    preview: presentation.preview,
    resource: Object.freeze(presentation.resource),
    secondaryResource: presentation.secondaryResource
      ? Object.freeze(presentation.secondaryResource)
      : null,
    runStatus: catalog.result.runStatus,
    connected,
    mediaActions,
    zoom: presentation.zoom ? Object.freeze(presentation.zoom) : null,
  });
}

/** Describe a presenter outcome that offers no retry or large-image admission. */
export function canvasResourceStatus(
  state: CanvasResourceStatus['state'],
  message: string,
): CanvasResourceStatus {
  return { state, message, canRetry: false, canAdmitLargeImage: false };
}

/**
 * Describe image B for an active comparison from the presenter's loaded B status.
 *
 * Unresolved or non-image selections are unavailable regardless of presenter progress; an
 * active comparison without a chosen B asks for one. No comparison has no B status.
 */
export function canvasSecondaryResource(
  plan: CanvasDisplayPlan,
  loaded: CanvasResourceStatus | undefined,
): CanvasResourceStatus | null {
  if (plan.secondary && (!plan.secondary.sourcePresent || !plan.secondary.item))
    return canvasResourceStatus('unavailable', 'The selected B image is unavailable.');
  if (plan.secondary?.item && plan.secondary.item.mediaKind !== 'image')
    return canvasResourceStatus('unavailable', 'Open B in the host viewer.');
  return plan.view.comparison
    ? (loaded ?? canvasResourceStatus('empty', 'Choose an output for B.'))
    : null;
}

/** Express B's physical scale per axis while B is stretched into A's image rectangle. */
export function canvasSecondaryPhysicalScale(
  physicalScale: number,
  primary: ImageDimensions,
  secondary: ImageDimensions,
): { readonly x: number; readonly y: number } {
  return Object.freeze({
    x: (physicalScale * primary.width) / secondary.width,
    y: (physicalScale * primary.height) / secondary.height,
  });
}
