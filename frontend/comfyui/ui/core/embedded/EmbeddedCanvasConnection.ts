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
import type { BrowserPanelTransport } from './BrowserPanelTransport.js';
import type { CanvasGeometryHost } from '../../../../../integrations/contracts/CanvasGeometry.js';
import type { CanvasSurfaceHost } from '../../../../../integrations/contracts/CanvasSurface.js';
import type { CanvasEditHost } from '../../../../../integrations/contracts/CanvasEditing.js';
import type { CanvasWorkflowHost } from '../../../../../integrations/contracts/CanvasWorkflowSurfaces.js';
/** Report whether native output discovery has concluded, without exposing the canvas session. */
export interface NativeSurfaceDiscovery {
  /** True once this page's native canvas session settled at least one context and surface discovery. */
  readonly surfaceDiscovered: boolean;
}

/** Keep the optional native canvas unavailable until the panel has authenticated and negotiated it. */
export class EmbeddedCanvasConnection implements CanvasLayerHost, NativeSurfaceDiscovery {
  /** Negotiate output presentation through this same canvas session. */
  get surface(): CanvasSurfaceHost | null {
    return this.#client?.surface ?? null;
  }
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
  /** Expose editing commands on the authenticated session; adapters gate them by capability. */
  get editing(): CanvasEditHost | null {
    return this.#client?.editing ?? null;
  }
  /** Expose workflow surface provisioning on the same correlated request sequence. */
  get workflow(): CanvasWorkflowHost | null {
    return this.#client?.workflow ?? null;
  }
  /** Expose mask transport only after the panel navigation has authenticated. */
  get masks(): CanvasMaskClient | null {
    return this.#masks;
  }
  #stop: (() => void) | null = null;
  #window: Window | null = null;
  #report: ((error: unknown) => void) | null = null;
  #read: object = {};
  #refreshing: Promise<void> | null = null;
  #refreshAgain = false;
  #surfaceDiscovered = false;
  #disposed = false;
  /** Report settled discovery, including a refused, unavailable or failed first context read. */
  get surfaceDiscovered(): boolean {
    return this.#surfaceDiscovered;
  }
  /** Read context and optional surface capabilities before publishing panel readiness. */
  connect(
    transport: BrowserPanelTransport,
    window: Window,
    report: (error: unknown) => void,
  ): void {
    if (this.#disposed) throw new Error('The native canvas connection is closed.');
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
    this.#window = window;
    this.#report = report;
    this.#masks = new CanvasMaskClient(
      transport.session,
      {
        send: (message) => transport.send(message),
        report,
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
    this.refresh();
  }
  /** Coalesce native discovery after an actual backend connection transition. */
  refresh(): void {
    const client = this.#client;
    if (!client) return;
    if (this.#refreshing) {
      this.#refreshAgain = true;
      return;
    }
    const pending = this.readContext().then(
      () => {},
      (error: unknown) => {
        if (this.#client === client) this.#report?.(error);
      },
    );
    this.#refreshing = pending;
    void pending.then(() => {
      if (this.#refreshing !== pending) return;
      this.#refreshing = null;
      if (this.#refreshAgain) {
        this.#refreshAgain = false;
        this.refresh();
      }
    });
  }
  /** Advertise only the capability descriptor received from the current native session. */
  readCanvas(): CanvasDescriptor | null {
    return this.#client?.readCanvas() ?? null;
  }
  /** Capture a native document identity before preparing an image transfer. */
  async readContext(): Promise<CanvasContext> {
    const client = this.#client;
    if (!client) throw new Error('The native canvas is unavailable.');
    const read = {};
    this.#read = read;
    let context: CanvasContext;
    try {
      context = await client.readContext();
    } finally {
      // Discovery concludes whether the host offered, refused or failed to describe a surface.
      if (this.#client === client && this.#read === read) {
        this.#surfaceDiscovered = true;
        this.#window?.document.dispatchEvent(new Event('sugarcubes-canvas-discovered'));
      }
    }
    if (this.#client === client && this.#read === read)
      this.#window?.document.dispatchEvent(new Event('sugarcubes-canvas-ready'));
    return context;
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
  /** Retire this page's connection permanently before releasing pending requests. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#stop?.();
    this.#stop = null;
    this.#client?.dispose();
    this.#client = null;
    this.#window = null;
    this.#report = null;
    this.#refreshing = null;
    this.#refreshAgain = false;
    this.#masks?.dispose();
    this.#masks = null;
  }
}
