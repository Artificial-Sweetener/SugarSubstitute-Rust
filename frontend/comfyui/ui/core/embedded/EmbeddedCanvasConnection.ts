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
/** Bind portable canvas operations to the authenticated panel navigation without claiming its transport. */
import {
  CanvasClient,
  CanvasMaskClient,
  type CanvasCancellationSource,
  type CanvasLayerHost,
  type CanvasContext,
  type CanvasInsertImage,
  type CanvasInsertResult,
  type CanvasDescriptor,
  type CanvasSourceHost,
} from './CanvasRuntime.js';
import type { BrowserPanelTransport } from './UxpPanelTransport.js';
import type { CanvasGeometryHost } from '../../../../../integrations/contracts/CanvasGeometry.js';
/** Keep the optional native canvas unavailable until the panel has authenticated and negotiated it. */
export class EmbeddedCanvasConnection implements CanvasLayerHost {
  /** Borrow the existing authenticated request sequence for optional geometry operations. */
  get geometry(): CanvasGeometryHost | null {
    return this.#client?.geometry ?? null;
  }
  #client: CanvasClient | null = null;
  #masks: CanvasMaskClient | null = null;
  /** Expose source reads on the authenticated canvas connection without a second request sequence. */
  get sources(): CanvasSourceHost | null {
    return this.#client?.sources ?? null;
  }
  /** Expose mask transport only after the panel navigation has authenticated. */
  get masks(): CanvasMaskClient | null {
    return this.#masks;
  }
  #stop: (() => void) | null = null;
  /** Start discovery once after panel configuration, including a bridge injected after page startup. */
  connect(
    transport: BrowserPanelTransport,
    window: Window,
    report: (error: unknown) => void,
  ): void {
    if (this.#client) return;
    const client = new CanvasClient(transport.session, {
      send: (message) => transport.send(message),
      later: (ms, callback) => {
        const timer = window.setTimeout(callback, ms);
        return () => window.clearTimeout(timer);
      },
      report,
    });
    this.#client = client;
    this.#masks = new CanvasMaskClient(
      transport.session,
      {
        send: (message) => transport.send(message),
        later: (ms, callback) => {
          const timer = window.setTimeout(callback, ms);
          return () => window.clearTimeout(timer);
        },
      },
      () => client.readCanvas()?.id ?? '',
    );
    this.#stop = transport.subscribe((value) => {
      client.receive(value);
      this.#masks?.receive(value);
    });
    void client
      .readContext()
      .then(() => window.document.dispatchEvent(new Event('sugarcubes-canvas-ready')))
      .catch(report);
  }
  /** Advertise only the capability descriptor received from the current native session. */
  readCanvas(): CanvasDescriptor | null {
    return this.#client?.readCanvas() ?? null;
  }
  /** Capture a native document identity before preparing an image transfer. */
  async readContext(): Promise<CanvasContext> {
    if (!this.#client) throw new Error('The native canvas is unavailable.');
    return await this.#client.readContext();
  }
  /** Delegate a mutation once to the correlated session client. */
  async insertImage(
    image: CanvasInsertImage,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasInsertResult> {
    if (!this.#client)
      return { status: 'unavailable', message: 'The native canvas is unavailable.' };
    return await this.#client.insertImage(image, cancellation);
  }
  /** Stop accepting replies before releasing pending requests at page teardown. */
  dispose(): void {
    this.#stop?.();
    this.#stop = null;
    this.#client?.dispose();
    this.#client = null;
    this.#masks?.dispose();
    this.#masks = null;
  }
}
