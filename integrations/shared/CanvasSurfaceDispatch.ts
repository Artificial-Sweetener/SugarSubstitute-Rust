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
/** Dispatch an optional surface operation without coupling it to editable document layers. */

import type { CanvasContext, CanvasMessage } from '../contracts/CanvasProtocol.generated.js';
import type { CanvasSurfaceReply } from '../contracts/CanvasSurfaceProtocol.generated.js';
import type { CanvasSurfaceHost, CanvasSurfaceResult } from '../contracts/CanvasSurface.js';
import type { CanvasCancellationSource } from '../contracts/CanvasHost.js';
import { isCanvasSurfaceReply } from './CanvasSurfaceValidation.generated.js';

/** The host receives only messages admitted by the shared canvas session and replay owner. */
export type CanvasSurfaceRequest = Extract<
  CanvasMessage,
  {
    type:
      | 'surface.prepare'
      | 'surface.geometry'
      | 'surface.select'
      | 'surface.commit'
      | 'surface.release';
  }
>;

/** Retain the validated wire value while asynchronous host work is pending. */
export function snapshotCanvasSurface(request: CanvasSurfaceRequest): CanvasSurfaceRequest {
  const address = Object.freeze({ ...request.address });
  if (request.type === 'surface.geometry') {
    return Object.freeze({
      ...request,
      address,
      geometry: Object.freeze({
        ...request.geometry,
        origin: Object.freeze({ ...request.geometry.origin }),
        viewport: Object.freeze({ ...request.geometry.viewport }),
        clip: Object.freeze({ ...request.geometry.clip }),
      }),
    });
  }
  if (request.type === 'surface.select') {
    return Object.freeze({ ...request, address, file: Object.freeze({ ...request.file }) });
  }
  return Object.freeze({ ...request, address });
}

/** Identify only the requests owned by the optional surface port. */
export function isCanvasSurfaceRequest(message: CanvasMessage): message is CanvasSurfaceRequest {
  return (
    message.type === 'surface.prepare' ||
    message.type === 'surface.geometry' ||
    message.type === 'surface.select' ||
    message.type === 'surface.commit' ||
    message.type === 'surface.release'
  );
}

/** Preserve the current session's cancellation across asynchronous context and native work. */
export async function dispatchCanvasSurface(
  request: CanvasSurfaceRequest,
  cancellation: CanvasCancellationSource,
  port: {
    readonly surface?: CanvasSurfaceHost | undefined;
    context(): Promise<CanvasContext>;
    report(error: unknown): void;
  },
): Promise<CanvasSurfaceReply> {
  const { protocol, version, session, requestId, ...command } = request;
  const envelope = {
    protocol,
    version,
    session,
    requestId,
    type: 'surface.reply',
    address: request.address,
    revision: request.revision,
  } as const;
  let result: CanvasSurfaceResult;
  try {
    const context = port.surface && !cancellation.cancelled() ? await port.context() : null;
    result = cancellation.cancelled()
      ? { status: 'cancelled', message: 'Canvas surface request cancelled.' }
      : !port.surface || !context?.canvas.capabilities.displayOutput
        ? { status: 'unavailable', message: 'This canvas does not support an output surface.' }
        : await port.surface.execute(command, cancellation);
    if (cancellation.cancelled())
      result = { status: 'cancelled', message: 'Canvas surface request cancelled.' };
  } catch (error: unknown) {
    if (cancellation.cancelled())
      result = { status: 'cancelled', message: 'Canvas surface request cancelled.' };
    else {
      port.report(error);
      result = { status: 'error', message: 'The canvas surface operation failed.' };
    }
  }
  const reply = { ...envelope, ...result };
  if (
    !isCanvasSurfaceReply(reply) ||
    (reply.status === 'ready' && request.type !== 'surface.prepare') ||
    (reply.status === 'applied' && request.type === 'surface.prepare')
  ) {
    port.report(new Error('Canvas surface returned an invalid result.'));
    return {
      ...envelope,
      status: 'error',
      message: 'The canvas surface returned an invalid result.',
    };
  }
  return reply;
}
