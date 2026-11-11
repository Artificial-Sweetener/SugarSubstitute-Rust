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
/** Describe shared controls from native presentation facts without borrowing native resources. */

import type { CanvasSurfaceViewportState } from '../../../../../integrations/contracts/CanvasSurface.js';
import type { OutputMediaProfile } from '../../output_saving/domain/OutputMedia.js';
import { canvasComparisonAvailable } from '../application/CanvasComparisonSelection.js';
import type { CanvasDisplayPlan } from '../application/CanvasDisplayPlan.js';
import {
  canvasSecondaryPhysicalScale,
  canvasSecondaryResource,
  canvasViewSnapshot,
} from '../application/CanvasViewSnapshot.js';
import type { CanvasCatalogSnapshot } from '../domain/CanvasCatalog.js';
import type { CanvasAttachmentAddress } from '../domain/CanvasIdentity.js';
import type {
  CanvasResourceStatus,
  CanvasSourceSummary,
  CanvasViewSnapshot,
} from '../domain/CanvasViewPort.js';
import type { NativeCanvasPresentationStatus } from './NativeCanvasStatus.js';
import type { CanvasOutputActions } from '../domain/CanvasMedia.js';

/** Collect the attachment-owned facts that one native snapshot projects. */
export interface NativeCanvasSnapshotState {
  readonly address: CanvasAttachmentAddress;
  readonly plan: CanvasDisplayPlan;
  readonly catalog: CanvasCatalogSnapshot;
  readonly sources: readonly CanvasSourceSummary[];
  readonly connected: boolean;
  readonly mediaActions: OutputMediaProfile | null;
  readonly itemActions?: readonly CanvasOutputActions[];
  readonly photoshopAllAvailable?: boolean;
  readonly presentation: NativeCanvasPresentationStatus;
  /** The host's camera for the presented pixels, including B's size while a pair is shown. */
  readonly viewport: CanvasSurfaceViewportState | null;
  /** Whether accepted Output live preview pixels are shown on this surface. */
  readonly preview: boolean;
  readonly previewAvailable: boolean;
  /**
   * Whether the Input route shows its document here. The Output preview is then not on screen, so
   * it locks no viewer control.
   */
  readonly input: boolean;
  /** Whether the host negotiated A/B pair selection and divider placement. */
  readonly comparison: boolean;
  /** Whether the host paints the overview grid's cells. */
  readonly overview: boolean;
  /** Native-frame material readiness, already fenced to this attachment and latest geometry. */
  readonly nativeBackdrop?: CanvasViewSnapshot['nativeBackdrop'];
}

/**
 * Publish only states the native surface can present.
 *
 * Without an overview host the surface presents image detail only, so overview and batch grids
 * stay hidden. Like the browser grid, a shown overview reports no image readiness, only native
 * failures. Without a negotiated comparison host, image B is withheld while the shared view's
 * comparison intent stays visible and escapable. A shown pair shares presentation readiness.
 */
export function nativeCanvasSnapshot(state: NativeCanvasSnapshotState): CanvasViewSnapshot {
  const { plan, viewport } = state;
  const item = plan.primary?.item;
  const presented: CanvasResourceStatus = {
    ...state.presentation,
    canRetry: false,
    canAdmitLargeImage: false,
  };
  let resource = presented;
  if (item && item.mediaKind !== 'image') {
    resource = {
      ...resource,
      state: 'unavailable',
      message: 'Open this media in the host viewer.',
    };
  } else if (!item && plan.preview && !state.previewAvailable) {
    resource = {
      ...resource,
      state: 'unavailable',
      message: 'Live previews are not available on this surface.',
    };
  } else if (
    plan.primary &&
    !item &&
    resource.state !== 'ready' &&
    !(plan.preview && state.previewAvailable)
  ) {
    resource = {
      ...resource,
      state: 'unavailable',
      message: 'The selected workflow output is unavailable.',
    };
  }
  const overview = state.overview && plan.view.mode === 'overview';
  if (overview && presented.state !== 'error' && presented.state !== 'unavailable')
    resource = { ...presented, state: 'empty', message: '' };
  else if (overview) resource = presented;
  const comparison = state.comparison ? plan.view.comparison : null;
  const secondaryDimensions = (comparison && viewport?.secondaryDimensions) || null;
  const snapshot = canvasViewSnapshot(
    state.address,
    plan,
    state.catalog,
    state.sources,
    state.connected,
    state.mediaActions,
    {
      ...(state.photoshopAllAvailable === undefined
        ? {}
        : { photoshopAllAvailable: state.photoshopAllAvailable }),
      comparisonAvailable: state.comparison && canvasComparisonAvailable(state.catalog),
      preview: state.preview && !state.input,
      resource,
      secondaryResource: comparison
        ? canvasSecondaryResource(plan, plan.secondary?.item ? presented : undefined)
        : null,
      zoom: viewport
        ? {
            mode: viewport.mode,
            physicalScale: viewport.physicalScale,
            dimensions: viewport.dimensions,
            secondaryPhysicalScale: secondaryDimensions
              ? canvasSecondaryPhysicalScale(
                  viewport.physicalScale,
                  viewport.dimensions,
                  secondaryDimensions,
                )
              : null,
            secondaryDimensions,
          }
        : null,
    },
    state.itemActions,
  );
  return Object.freeze({
    ...snapshot,
    nativeBackdrop: state.nativeBackdrop ?? 'unavailable',
    secondary: comparison ? snapshot.secondary : null,
    grid: overview ? snapshot.grid : Object.freeze([]),
    view: Object.freeze({
      ...snapshot.view,
      mode: overview ? ('overview' as const) : ('image' as const),
      comparison: plan.view.comparison,
    }),
  });
}
