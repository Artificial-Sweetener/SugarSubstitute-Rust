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
/** Present a selected output through a host-owned surface without inserting an editable layer. */

import type { CanvasCancellationSource } from './CanvasHost.js';
import type { CanvasChangeObservation } from './CanvasChangeObservation.js';
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommitRequest,
  CanvasSurfaceGeometryRequest,
  CanvasSurfacePrepareRequest,
  CanvasSurfaceReleaseRequest,
  CanvasSurfaceReply,
  CanvasSurfaceSelectRequest,
  CanvasSurfaceOverviewRequest,
  CanvasSurfaceViewportRequest,
  CanvasSurfaceViewportReply,
  CanvasSurfacePreviewBeginRequest,
  CanvasSurfacePreviewChunkRequest,
  CanvasSurfacePreviewEndRequest,
  CanvasSurfacePreviewWithdrawRequest,
  CanvasSurfacePresentationChange,
} from './CanvasSurfaceProtocol.generated.js';

export type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceOverviewCell,
  CanvasSurfaceViewportCommand,
  CanvasSurfaceViewportState,
  CanvasSurfacePreviewIdentity,
  CanvasSurfacePresentationChange,
  CanvasSurfacePresentationDisplay,
} from './CanvasSurfaceProtocol.generated.js';

type Envelope = 'protocol' | 'version' | 'session' | 'requestId';

/** Retain the exact attachment and revision throughout preparation, presentation and release. */
export type CanvasSurfaceCommand =
  | Omit<CanvasSurfacePrepareRequest, Envelope>
  | Omit<CanvasSurfaceGeometryRequest, Envelope>
  | Omit<CanvasSurfaceSelectRequest, Envelope>
  | Omit<CanvasSurfaceOverviewRequest, Envelope>
  | Omit<CanvasSurfaceCommitRequest, Envelope>
  | Omit<CanvasSurfaceReleaseRequest, Envelope>
  | Omit<CanvasSurfaceViewportRequest, Envelope>
  | Omit<CanvasSurfacePreviewBeginRequest, Envelope>
  | Omit<CanvasSurfacePreviewChunkRequest, Envelope>
  | Omit<CanvasSurfacePreviewEndRequest, Envelope>
  | Omit<CanvasSurfacePreviewWithdrawRequest, Envelope>;

/** Return an explicit host outcome without exposing native resources or filesystem authority. */
export type CanvasSurfaceResult =
  | Pick<CanvasSurfaceReply, 'status' | 'message'>
  | Pick<CanvasSurfaceViewportReply, 'status' | 'message' | 'viewport'>;

/** Negotiate and operate an optional presentation surface using the existing canvas session. */
export interface CanvasSurfaceHost {
  /** Describe implemented presentation operations; this declaration grants no native authority. */
  readonly capabilities: CanvasSurfaceCapabilities;
  /**
   * Observe lease-correlated native presentation through the existing change subscription.
   * Preview transfer requires an accepted observation subscription. Its final release fences
   * callbacks, cancels incomplete preview work and requests restoration of retained final custody.
   * Older hosts omit this capability. Notifications grant no native authority.
   */
  readonly presentation?: CanvasChangeObservation<CanvasSurfacePresentationChange>;
  /**
   * Prepare a hidden candidate, select its output, commit it, update geometry or release it.
   * A successful selection on a committed surface, or a successful commit, acknowledges that
   * the corresponding selected frame has been accepted by the native presentation queue.
   * A successful visible geometry update on a committed surface with a selected image
   * acknowledges a correctly sized frame. A successful hidden geometry update acknowledges
   * that the surface accepted the hide.
   * Queue acceptance does not assert compositor completion or physical display scanout.
   * A negotiated viewport request reads or changes the native camera. Zoom factors multiply
   * the current scale around the viewport center; native centers physical 1:1. Pan uses
   * viewport fractions. Successful viewport replies describe the settled camera represented
   * by an accepted native frame. Empty or hidden surfaces return unavailable without a snapshot.
   * Preview begin/chunk/end acknowledgements transfer bounded byte custody only. Each step
   * releases its request; preparation and native acceptance arrive through presentation.
   * The encoded transfer ceiling is 64 MiB, with independent stricter host memory/decoder
   * admission. A lease expires 30 seconds after begin; chunks and progress never renew it.
   * A token binds immutable bytes and preview identity to begin's revision, independently
   * of later camera revisions. Withdrawal revokes that preview episode and restores retained
   * final custody asynchronously. Canceling an acknowledged request does not replace withdrawal.
   * Preview-capable hosts can commit a prepared empty surface, acknowledging empty native state.
   * A first preview needs no selected final. Only accepted preview pixels lock navigation to Fit;
   * the final remains navigable while preview bytes and pixels are being prepared.
   * Comparison requires the declared comparison capability and the wipe viewport command. A
   * selection carrying `comparison` presents A and B together under that selection's one
   * revision; omitting it presents A alone. B is stretched into A's image rectangle and shares
   * its camera. Wipe shows B within the left (vertical) or top (horizontal) 0..1 `split`
   * fraction of the viewport, and is unavailable without a shown comparison.
   * Viewport state carries B's dimensions only while a comparison is shown.
   * Overview requires the declared overview capability. It presents one to eight distinct
   * results, each fitted (contain) into its whole `rect`: CSS pixels relative to the geometry
   * origin. A partially scrolled cell keeps its full rect, which may start at a negative origin;
   * the host clips painting to the viewport. Its applied reply follows native presentation of the whole grid, and like
   * a selection it fences preview custody and becomes the displayed selection revision. The grid
   * has no camera, so viewport requests are unavailable while it is shown; a later selection
   * leaves it. An encoded overview message must fit the 32 KiB native surface message ceiling.
   */
  execute(
    command: CanvasSurfaceCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasSurfaceResult>;
}
