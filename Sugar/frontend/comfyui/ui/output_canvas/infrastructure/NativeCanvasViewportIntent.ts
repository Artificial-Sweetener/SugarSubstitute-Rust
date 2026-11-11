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
/** Settle one scoped viewport intent without owning a native operation queue or transport. */
import type {
  CanvasSurfaceResult,
  CanvasSurfaceViewportCommand,
} from '../../../../../integrations/contracts/CanvasSurface.js';
import type { CanvasOperationScope } from '../domain/CanvasViewPort.js';

/** Keep promise settlement and abort-listener custody independent of native stream sequencing. */
export class NativeCanvasViewportIntent {
  readonly result: Promise<boolean>;
  running = false;
  #settled = false;
  #resolve!: (applied: boolean) => void;

  /** Retain exactly one caller scope while the presentation owner decides when to forward it. */
  constructor(
    readonly command: CanvasSurfaceViewportCommand,
    readonly scope: CanvasOperationScope,
    private readonly cancel: (intent: NativeCanvasViewportIntent) => void,
    private readonly finished: (intent: NativeCanvasViewportIntent) => void,
  ) {
    this.result = new Promise<boolean>((resolve) => {
      this.#resolve = resolve;
    });
  }

  /** Observe cancellation only after the presentation owner has installed this intent. */
  observe(): void {
    this.scope.signal.addEventListener('abort', this.#cancel, { once: true });
    if (!this.scope.current()) this.complete(false);
  }

  /** Release the caller listener and report one terminal result without releasing native credit. */
  complete(applied: boolean): void {
    if (this.#settled) return;
    this.#settled = true;
    this.scope.signal.removeEventListener('abort', this.#cancel);
    this.finished(this);
    this.#resolve(applied);
  }

  /** Delegate cancellation policy to the sole native operation owner. */
  readonly #cancel = (): void => {
    this.cancel(this);
  };
}

/** Name what the presentation owner does after the host refuses one viewport request. */
export interface NativeCanvasViewportRejection {
  /** Keep the shown pixels and read the camera now shown instead of failing the command. */
  readonly recover: boolean;
  /** Dirty the stream again so an admitted command is retried, recovered or reconciled. */
  readonly retry: boolean;
  /** A superseded read leaves the newer gesture's settled camera to report itself. */
  readonly settled: boolean;
  /** Preview ownership settles this camera through its correlated display observation. */
  readonly parked: boolean;
  /** Publish the refusal as a presentation failure. */
  readonly failed: boolean;
}

/**
 * Classify a refused viewport request.
 *
 * A stale answer while no preview transfer or pixels claim the surface means a newer camera, such
 * as a wheel gesture, replaced the destination: the pixels stay, the superseded command did not
 * apply, and one read reports the camera now shown. A host that could not apply a command is
 * recovered the same way. While a preview owns the camera, a stale answer is parked without a
 * failure because the preview's display observation settles it.
 */
export function viewportRejection(
  status: CanvasSurfaceResult['status'],
  admitted: boolean,
  current: boolean,
  previewOwnsCamera: boolean,
): NativeCanvasViewportRejection {
  const superseded = status === 'stale';
  const recover = admitted && (status === 'unavailable' || (superseded && !previewOwnsCamera));
  return Object.freeze({
    recover,
    retry: admitted && (!current || status === 'cancelled' || recover),
    settled: current && superseded && !previewOwnsCamera && !admitted,
    parked: current && superseded && previewOwnsCamera,
    failed: current && status !== 'cancelled' && !recover && !superseded,
  });
}
