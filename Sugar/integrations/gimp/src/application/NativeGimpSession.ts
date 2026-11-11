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
/** Own private native request cancellation independently of the companion's WebView framework. */
import type { CanvasCancellationSource } from '../../../contracts/CanvasHost.js';
import { CanvasCancellation } from '../../../shared/CanvasCancellation.js';
import type { NativeCommand, NativeResult } from '../contracts/NativeProtocol.generated.js';

/** Implement native IO in the viewer adapter while keeping SDK operations private to GIMP. */
export interface NativeGimpTransport {
  submit(command: NativeCommand): Promise<number>;
  wait(id: number): Promise<NativeResult>;
  cancel(id: number): Promise<void>;
  close(): Promise<void>;
}

interface Request {
  id: number | null;
  readonly cancellation: CanvasCancellation;
  cancelling: Promise<void> | null;
}

/** Translate native primitives through any viewer while retaining the existing canvas protocol above it. */
export interface GimpNativePort {
  execute(command: NativeCommand, cancellation?: CanvasCancellationSource): Promise<NativeResult>;
}

/** Keep cancelled tickets and offscreen resources owned across asynchronous native admission. */
export class NativeGimpSession implements GimpNativePort {
  readonly #requests = new Set<Request>();
  #closed = false;

  /** Borrow typed IO without depending on Tauri, browser globals or the GIMP SDK. */
  constructor(
    private readonly port: NativeGimpTransport,
    private readonly report: (error: unknown) => void,
  ) {}

  /** Send one primitive once and preserve an insertion that already crossed its native commit point. */
  async execute(
    command: NativeCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<NativeResult> {
    if (this.#closed || cancellation?.cancelled()) return cancelled();
    const request: Request = { id: null, cancellation: new CanvasCancellation(), cancelling: null };
    this.#requests.add(request);
    const stop =
      cancellation?.subscribe(() => {
        request.cancellation.cancel();
      }) ?? (() => {});
    const stopNative = request.cancellation.subscribe(() => {
      void this.#cancel(request);
    });
    try {
      if (request.cancellation.cancelled()) return cancelled();
      request.id = await this.port.submit(command);
      if (request.cancellation.cancelled()) await this.#cancel(request);
      const result = await this.port.wait(request.id);
      if (request.cancellation.cancelled() && !('committed' in result)) {
        await this.#cancel(request);
        return cancelled();
      }
      return result;
    } catch (error: unknown) {
      await this.#cancel(request);
      if (request.cancellation.cancelled()) return cancelled();
      throw error;
    } finally {
      stop();
      stopNative();
      this.#requests.delete(request);
    }
  }

  /** Retire this renderer generation, including snapshots whose successful replies already completed. */
  dispose(): void {
    if (this.#closed) return;
    this.#closed = true;
    for (const request of this.#requests) request.cancellation.cancel();
    void this.port.close().catch(this.report);
  }

  /** Forward cancellation once after a ticket exists, including cancellation during submission. */
  #cancel(request: Request): Promise<void> {
    if (request.id === null) return Promise.resolve();
    request.cancelling ??= this.port.cancel(request.id).catch(this.report);
    return request.cancelling;
  }
}

/** Keep native cancellation values compatible with the generated private failure union. */
function cancelled(): NativeResult {
  return { kind: 'failure', status: 'cancelled', message: 'GIMP native request cancelled.' };
}
