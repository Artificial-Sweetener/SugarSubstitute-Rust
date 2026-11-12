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
  CanvasSurfaceCameraChange,
} from './CanvasSurfaceProtocol.generated.js';

export type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceBackdrop,
  CanvasSurfaceBackdropSupport,
  CanvasSurfaceOcclusion,
  CanvasSurfaceOverviewCell,
  CanvasSurfaceViewportCommand,
  CanvasSurfaceViewportState,
  CanvasSurfaceInspection,
  CanvasSurfaceCameraChange,
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

/** Deliver settled host-routed gesture cameras to listeners until each one unsubscribes. */
export interface CanvasSurfaceCameraObservation {
  listen(listener: (change: CanvasSurfaceCameraChange) => void): () => void;
}

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
   * Observe the settled camera after a gesture the host routed to its own navigation tool.
   * Requires the declared inspect command and arrives only while presentation is observed:
   * once per settled or cancelled gesture, never per move, and never for previews or overview
   * grids. Its sequence shares the presentation sequence; selectionRevision names the shown
   * final selection. Notifications grant no native authority.
   */
  readonly camera?: CanvasSurfaceCameraObservation;
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
   * Native surface discovery uses schema 2 within canvas protocol version 3. Schema 1 is
   * recognized only to return an unsupported discovery result; matching host and client
   * upgrades are required before any surface mutation. Camera notifications use schema 2;
   * presentation observation retains its independent schema 1.
   * Inspection requires the declared `inspect` viewport command. Its finite `center` is the
   * image-relative point at the viewport center and can lie outside the image on either axis.
   * Its positive `span` is normalized image distance per physical display pixel, independent
   * of viewport size. A selection carrying an inspection presents its first frame at that
   * camera; inspect applies one immediately without animation. The host contains differing
   * axis spans, preserves the center and representable scale without interactive pan/zoom
   * clamping, and reports the actual accepted center in every viewport state. Unrepresentable
   * transforms fail without replacing the preceding camera or authored document.
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
   * The input capability declares that the host routes native canvas gestures from the OS
   * against page-published occlusions. Only such a host receives geometry `occlusions`: up to 64
   * possibly overlapping rects, in CSS pixels relative to the geometry origin, where web content
   * (toolbars, tab strips, menus, popups, dialogs, overlays) covers the native viewport; and
   * `modal`, true while a modal dialog or overlay blocks the whole canvas. Occlusion changes
   * travel as ordinary acknowledged geometry updates.
   */
  execute(
    command: CanvasSurfaceCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasSurfaceResult>;
}
