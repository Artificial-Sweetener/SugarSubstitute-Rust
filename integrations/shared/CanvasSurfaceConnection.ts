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
/** Own native surface grants independently of same-session discovery queries. */

import type { CanvasCancellationSource } from '../contracts/CanvasHost.js';
import type { CanvasDescriptor, CanvasMessage } from '../contracts/CanvasProtocol.generated.js';
import type {
  CanvasSurfaceCameraChange,
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
  CanvasSurfaceHost,
  CanvasSurfacePresentationChange,
  CanvasSurfaceResult,
} from '../contracts/CanvasSurface.js';
import type { CanvasSurfacePresentationReply } from '../contracts/CanvasSurfaceProtocol.generated.js';
import type { CanvasRemoteChanges } from './CanvasRemoteChanges.js';
import { discoverCanvasSurface } from './CanvasSurfaceDiscovery.js';
import { canvasSurfacePresentation } from './CanvasSurfacePresentation.js';

/** Borrow the session's correlated requests and event streams without owning another transport. */
export interface CanvasSurfaceConnectionPort {
  read(): Promise<CanvasMessage>;
  execute(
    command: CanvasSurfaceCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasSurfaceResult>;
  watch(enabled: boolean): Promise<CanvasSurfacePresentationReply>;
  presentation(listener: (change: CanvasSurfacePresentationChange) => void): () => void;
  camera(listener: (change: CanvasSurfaceCameraChange) => void): () => void;
  report(error: unknown): void;
}

interface SurfaceGrant {
  readonly canvas: string;
  readonly presentation: CanvasRemoteChanges<CanvasSurfacePresentationChange>;
  surface: CanvasSurfaceHost | null;
  valid: boolean;
}

/** Commit replacement grants atomically while identical validated refreshes retain their frame. */
export class CanvasSurfaceConnection {
  #active: SurfaceGrant | null = null;

  /** Receive only portable session operations whose identity, deadlines and cancellation are owned. */
  constructor(private readonly port: CanvasSurfaceConnectionPort) {}

  /** Expose the active executable grant, including while its unchanged canvas is being queried. */
  get surface(): CanvasSurfaceHost | null {
    return this.#active?.surface ?? null;
  }

  /** Revoke the grant before releasing its observation; stale proxies retain teardown only. */
  withdraw(): void {
    const previous = this.#active;
    this.#active = null;
    if (!previous) return;
    previous.valid = false;
    previous.presentation.dispose();
  }

  /**
   * Negotiate the latest query without withdrawing an unchanged active canvas in advance.
   *
   * A changed canvas descriptor withdraws immediately. A refused or changed capability reply
   * replaces the old grant at commit. The caller's latest-query fence prevents an obsolete
   * candidate from publishing after another read, explicit withdrawal, or session disposal.
   */
  async refresh(canvas: CanvasDescriptor, current: () => boolean): Promise<void> {
    if (!current()) return;
    const identity = canvasDescriptorKey(canvas);
    if (this.#active?.canvas !== identity) this.withdraw();
    if (!canvas.capabilities.displayOutput) {
      this.withdraw();
      return;
    }
    const owns = (): boolean => candidate.valid && (current() || this.#active === candidate);
    const presentation = canvasSurfacePresentation({
      current: owns,
      listen: (listener) => this.port.presentation(listener),
      watch: (enabled) => this.port.watch(enabled),
      report: (error) => {
        this.port.report(error);
      },
    });
    const candidate: SurfaceGrant = { canvas: identity, presentation, surface: null, valid: true };
    const surface = await discoverCanvasSurface({
      read: () => this.port.read(),
      current: owns,
      owns: (host) => this.#active === candidate && candidate.surface === host,
      presentation,
      camera: { listen: (listener) => this.port.camera(listener) },
      observesPresentation: () => presentation.observing,
      execute: (command, cancellation) => this.port.execute(command, cancellation),
      report: (error) => {
        this.port.report(error);
      },
    });
    if (!current()) {
      candidate.valid = false;
      presentation.dispose();
      return;
    }
    if (
      surface &&
      this.surface &&
      surfaceCapabilitiesKey(surface.capabilities) ===
        surfaceCapabilitiesKey(this.surface.capabilities)
    ) {
      candidate.valid = false;
      presentation.dispose();
      return;
    }
    this.withdraw();
    if (surface) {
      candidate.surface = surface;
      this.#active = candidate;
    } else {
      candidate.valid = false;
      presentation.dispose();
    }
  }
}

/** Compare validated descriptors by value, independently of wire object property order. */
function canvasDescriptorKey(canvas: CanvasDescriptor): string {
  const capabilities = canvas.capabilities;
  return JSON.stringify([
    canvas.id,
    canvas.label,
    capabilities.displayOutput,
    capabilities.insertImageLayer,
    capabilities.positionImageLayer,
    'geometry' in capabilities ? capabilities.geometry : null,
    'editing' in capabilities ? capabilities.editing : null,
    'workflowSurfaces' in capabilities ? capabilities.workflowSurfaces : null,
  ]);
}

/** Compare the complete negotiated grant without deriving new capability policy. */
function surfaceCapabilitiesKey(capabilities: CanvasSurfaceCapabilities): string {
  return JSON.stringify([
    capabilities.schema,
    capabilities.imageFormats,
    capabilities.viewportCommands,
    capabilities.viewportState,
    capabilities.previewFormats,
    capabilities.comparison,
    capabilities.overview,
    capabilities.input,
    capabilities.backdrop?.schema ?? null,
  ]);
}
