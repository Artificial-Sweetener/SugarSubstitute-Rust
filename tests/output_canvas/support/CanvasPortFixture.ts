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
/** Provide typed surface-port fixtures without a browser image or Comfy runtime. */

import {
  outputSourceId,
  workflowSessionId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import {
  canvasSessionId,
  canvasViewId,
  surfaceAttachmentId,
  windowId,
  type CanvasAttachmentAddress,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasIdentity.js';
import { initialCanvasViewState } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewState.js';
import type {
  CanvasCapabilities,
  CanvasCommandOutcome,
  CanvasCommandRequest,
  CanvasSurfaceGeometry,
  CanvasViewPort,
  CanvasViewSnapshot,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';

/** Construct a contract-only state source; its status is supplied by the surface authority. */
export function canvasSnapshot(key: string): CanvasViewSnapshot {
  const address: CanvasAttachmentAddress = {
    sessionId: canvasSessionId(workflowSessionId(key)),
    viewId: canvasViewId(key),
    windowId: windowId(key),
    attachmentId: surfaceAttachmentId(key),
    generation: 1,
  };
  return {
    schema: 1,
    address,
    revision: 1,
    view: {
      ...initialCanvasViewState(address),
      selection: { sourceId: outputSourceId('7'), batchIndex: 2 },
    },
    sources: [
      {
        id: outputSourceId('7'),
        label: 'Saved image',
        breadcrumb: [],
        batchCount: 3,
        preview: false,
        previousRun: false,
      },
      {
        id: outputSourceId('42:23'),
        label: 'Detail',
        breadcrumb: ['Scene'],
        batchCount: 3,
        preview: false,
        previousRun: false,
      },
    ],
    selected: null,
    secondary: null,
    preview: false,
    zoom: {
      mode: 'fit',
      physicalScale: 0.5,
      dimensions: { width: 4096, height: 2160 },
      secondaryPhysicalScale: null,
      secondaryDimensions: null,
    },
    resource: { state: 'empty', message: '', canRetry: false, canAdmitLargeImage: false },
    secondaryResource: null,
    runStatus: 'idle',
    connected: true,
  };
}

/** Exercise overlay commands and delayed delivery without pretending to render native images. */
export class CanvasPortFixture implements CanvasViewPort {
  readonly capabilities: CanvasCapabilities;
  readonly requests: CanvasCommandRequest[] = [];
  readonly geometries: Array<{
    address: CanvasAttachmentAddress;
    geometry: CanvasSurfaceGeometry;
  }> = [];
  readonly releases: CanvasAttachmentAddress[] = [];
  readonly #listeners = new Set<(snapshot: CanvasViewSnapshot) => void>();
  readonly #pending = new Map<number, (outcome: CanvasCommandOutcome) => void>();
  #snapshot: CanvasViewSnapshot;
  delayed = false;
  failure: Error | null = null;

  /** Supply distinct capabilities and authoritative status for each contract fixture. */
  constructor(
    snapshot: CanvasViewSnapshot,
    commands: CanvasCapabilities['commands'] = [
      'select',
      'select-source',
      'batch',
      'follow',
      'overview',
      'link',
      'fit',
      'native',
      'zoom',
      'pan',
      'comparison',
      'select-comparison',
      'swap',
      'wipe',
      'retry',
      'admit-large-image',
      'open-media',
    ],
  ) {
    this.#snapshot = snapshot;
    this.capabilities = { schema: 1, commands };
  }

  /** Count consumers so lifecycle tests can detect leaked subscriptions. */
  get listenerCount(): number {
    return this.#listeners.size;
  }

  /** Read the fixture authority's current snapshot. */
  read(): CanvasViewSnapshot {
    return this.#snapshot;
  }

  /** Register a root-scoped subscriber. */
  subscribe(listener: (snapshot: CanvasViewSnapshot) => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Record intent and optionally delay acknowledgment past root disposal. */
  async dispatch(request: CanvasCommandRequest): Promise<CanvasCommandOutcome> {
    this.requests.push(request);
    if (this.failure) throw this.failure;
    if (this.delayed)
      return await new Promise((resolve) => this.#pending.set(request.requestId, resolve));
    return { status: 'applied', revision: this.#snapshot.revision };
  }

  /** Record explicit surface geometry without mounting image resources. */
  updateSurface(address: CanvasAttachmentAddress, geometry: CanvasSurfaceGeometry): void {
    this.geometries.push({ address, geometry });
  }

  /** Record releases to prove that overlay teardown does not dispose the surface. */
  releaseSurface(address: CanvasAttachmentAddress): void {
    this.releases.push(address);
  }

  /** Deliver arbitrary typed orderings for attachment and revision tests. */
  publish(snapshot: CanvasViewSnapshot): void {
    this.#snapshot = snapshot;
    for (const listener of this.#listeners) listener(snapshot);
  }

  /** Settle one outstanding fixture command. */
  settle(requestId: number, outcome: CanvasCommandOutcome): void {
    const finish = this.#pending.get(requestId);
    if (!finish) throw new Error('No fixture command is pending.');
    this.#pending.delete(requestId);
    finish(outcome);
  }
}
