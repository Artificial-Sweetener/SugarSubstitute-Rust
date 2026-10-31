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
/** Discover presentation support through the existing canvas session without granting host authority. */
import type { CanvasCancellationSource } from '../contracts/CanvasHost.js';
import type { CanvasContext, CanvasMessage } from '../contracts/CanvasProtocol.generated.js';
import type { CanvasSurfaceHost, CanvasSurfaceCapabilities } from '../contracts/CanvasSurface.js';
import type {
  CanvasSurfaceCapabilitiesRequest,
  CanvasSurfaceCapabilitiesReply,
} from '../contracts/CanvasSurfaceProtocol.generated.js';
import type { CanvasSurfaceCommand, CanvasSurfaceResult } from '../contracts/CanvasSurface.js';
import { isCanvasSurfaceCapabilities } from './CanvasSurfaceValidation.generated.js';
import {
  negotiatesCanvasSurfaceInspection,
  supportsCanvasSurfaceViewport,
} from './CanvasSurfaceViewport.js';
import { supportsCanvasSurfaceComparison } from './CanvasSurfaceComparison.js';
import { supportsCanvasSurfaceOverview } from './CanvasSurfaceOverview.js';
import { supportsCanvasSurfaceOcclusion } from './CanvasSurfaceOcclusion.js';
import type { CanvasChangeObservation } from '../contracts/CanvasChangeObservation.js';
import type {
  CanvasSurfaceCameraObservation,
  CanvasSurfacePresentationChange,
} from '../contracts/CanvasSurface.js';
import { isCanvasPreviewCommand, supportsCanvasSurfacePreview } from './CanvasSurfacePreview.js';

/** Represent an explicit refusal without suggesting any executable operation. */
export const EMPTY_CANVAS_SURFACE_CAPABILITIES: CanvasSurfaceCapabilities = Object.freeze({
  schema: 1,
  imageFormats: Object.freeze([]),
  viewportCommands: Object.freeze([]),
  viewportState: false,
  previewFormats: Object.freeze([]),
  comparison: false,
  overview: false,
  input: false,
});

/** Reject advertised operations accompanying a refusal. */
export function emptyCanvasSurfaceCapabilities(value: CanvasSurfaceCapabilities): boolean {
  return (
    value.imageFormats.length === 0 &&
    value.viewportCommands.length === 0 &&
    !value.viewportState &&
    value.previewFormats.length === 0 &&
    !value.comparison &&
    !value.overview &&
    !value.input
  );
}

/** Retain immutable declarations when an adapter or transport later mutates its source value. */
export function snapshotCanvasSurfaceCapabilities(
  value: CanvasSurfaceCapabilities,
): CanvasSurfaceCapabilities {
  return Object.freeze({
    ...value,
    imageFormats: Object.freeze([...value.imageFormats]),
    viewportCommands: Object.freeze([...value.viewportCommands]),
    previewFormats: Object.freeze([...value.previewFormats]),
  });
}

/** Describe only a currently available surface while preserving the session's cancellation owner. */
export async function dispatchCanvasSurfaceDiscovery(
  request: CanvasSurfaceCapabilitiesRequest,
  cancellation: CanvasCancellationSource,
  port: {
    readonly surface?: CanvasSurfaceHost | undefined;
    context(): Promise<CanvasContext>;
    report(error: unknown): void;
  },
): Promise<CanvasSurfaceCapabilitiesReply> {
  const envelope = {
    protocol: request.protocol,
    version: request.version,
    session: request.session,
    requestId: request.requestId,
    type: 'surface.capabilities.reply',
  } as const;
  const empty = EMPTY_CANVAS_SURFACE_CAPABILITIES;
  try {
    if (!port.surface)
      return {
        ...envelope,
        status: 'unsupported',
        capabilities: empty,
        message: 'This canvas does not provide an output surface.',
      };
    const context = await port.context();
    if (cancellation.cancelled() || !context.canvas.capabilities.displayOutput)
      return {
        ...envelope,
        status: 'unavailable',
        capabilities: empty,
        message: 'The canvas surface is unavailable.',
      };
    const capabilities = port.surface.capabilities;
    if (!isCanvasSurfaceCapabilities(capabilities))
      throw new Error('Canvas surface declared invalid capabilities.');
    return {
      ...envelope,
      status: 'available',
      capabilities: snapshotCanvasSurfaceCapabilities(capabilities),
      message: '',
    };
  } catch (error: unknown) {
    port.report(error);
    return {
      ...envelope,
      status: 'error',
      capabilities: empty,
      message: 'Canvas surface discovery failed.',
    };
  }
}

/** Negotiate support while leaving context identity and the executable proxy with the session owner. */
export async function discoverCanvasSurface(port: {
  read(): Promise<CanvasMessage>;
  current(): boolean;
  owns(host: CanvasSurfaceHost): boolean;
  readonly presentation?: CanvasChangeObservation<CanvasSurfacePresentationChange>;
  readonly camera?: CanvasSurfaceCameraObservation;
  observesPresentation?(): boolean;
  execute(
    command: CanvasSurfaceCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasSurfaceResult>;
  report(error: unknown): void;
}): Promise<CanvasSurfaceHost | null> {
  try {
    const reply = await port.read();
    if (!port.current()) return null;
    if (reply.type !== 'surface.capabilities.reply')
      throw new Error('Invalid canvas surface capability reply.');
    if (reply.status !== 'available') {
      if (!emptyCanvasSurfaceCapabilities(reply.capabilities))
        throw new Error('Unavailable canvas surface declared capabilities.');
      if (reply.status === 'error')
        port.report(new Error(reply.message || 'Canvas surface discovery failed.'));
      return null;
    }
    const host: CanvasSurfaceHost = Object.freeze<CanvasSurfaceHost>({
      capabilities: snapshotCanvasSurfaceCapabilities(reply.capabilities),
      ...(port.presentation && reply.capabilities.previewFormats.length
        ? { presentation: port.presentation }
        : {}),
      ...(port.camera && negotiatesCanvasSurfaceInspection(reply.capabilities)
        ? { camera: port.camera }
        : {}),
      execute: (command, cancellation) =>
        command.type === 'surface.release' ||
        command.type === 'surface.preview.withdraw' ||
        (port.current() && port.owns(host))
          ? supportsCanvasSurfaceViewport(host.capabilities, command) &&
            supportsCanvasSurfaceComparison(host.capabilities, command) &&
            supportsCanvasSurfaceOverview(host.capabilities, command) &&
            supportsCanvasSurfaceOcclusion(host.capabilities, command) &&
            supportsCanvasSurfacePreview(host.capabilities, command) &&
            (!isCanvasPreviewCommand(command) ||
              command.type === 'surface.preview.withdraw' ||
              port.observesPresentation?.())
            ? port.execute(command, cancellation)
            : Promise.resolve({
                status: 'unavailable',
                message: 'Canvas surface operation is unavailable.',
              })
          : Promise.resolve({ status: 'unavailable', message: 'The canvas context changed.' }),
    });
    return host;
  } catch (error: unknown) {
    if (port.current()) port.report(error);
    return null;
  }
}
