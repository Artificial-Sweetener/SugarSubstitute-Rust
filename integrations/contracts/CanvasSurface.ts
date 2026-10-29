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
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommitRequest,
  CanvasSurfaceGeometryRequest,
  CanvasSurfacePrepareRequest,
  CanvasSurfaceReleaseRequest,
  CanvasSurfaceReply,
  CanvasSurfaceSelectRequest,
  CanvasSurfaceViewportRequest,
  CanvasSurfaceViewportReply,
} from './CanvasSurfaceProtocol.generated.js';

export type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceViewportCommand,
  CanvasSurfaceViewportState,
} from './CanvasSurfaceProtocol.generated.js';

type Envelope = 'protocol' | 'version' | 'session' | 'requestId';

/** Retain the exact attachment and revision throughout preparation, presentation and release. */
export type CanvasSurfaceCommand =
  | Omit<CanvasSurfacePrepareRequest, Envelope>
  | Omit<CanvasSurfaceGeometryRequest, Envelope>
  | Omit<CanvasSurfaceSelectRequest, Envelope>
  | Omit<CanvasSurfaceCommitRequest, Envelope>
  | Omit<CanvasSurfaceReleaseRequest, Envelope>
  | Omit<CanvasSurfaceViewportRequest, Envelope>;

/** Return an explicit host outcome without exposing native resources or filesystem authority. */
export type CanvasSurfaceResult =
  | Pick<CanvasSurfaceReply, 'status' | 'message'>
  | Pick<CanvasSurfaceViewportReply, 'status' | 'message' | 'viewport'>;

/** Negotiate and operate an optional presentation surface using the existing canvas session. */
export interface CanvasSurfaceHost {
  /** Describe implemented presentation operations; this declaration grants no native authority. */
  readonly capabilities: CanvasSurfaceCapabilities;
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
   */
  execute(
    command: CanvasSurfaceCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasSurfaceResult>;
}
