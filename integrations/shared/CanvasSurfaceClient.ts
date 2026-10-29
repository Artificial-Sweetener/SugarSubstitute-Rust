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
/** Validate attachment operation replies while the canvas session retains request and deadline ownership. */
import type { CanvasCancellationSource } from '../contracts/CanvasHost.js';
import type { CanvasMessage } from '../contracts/CanvasProtocol.generated.js';
import type { CanvasSurfaceCommand, CanvasSurfaceResult } from '../contracts/CanvasSurface.js';
import type { CanvasSurfaceAddress } from '../contracts/CanvasSurfaceProtocol.generated.js';
import { snapshotCanvasSurfaceViewport } from './CanvasSurfaceViewport.js';

/** Require the exact attachment and revision in the host's correlated surface reply. */
export async function requestCanvasSurface(
  command: CanvasSurfaceCommand,
  cancellation: CanvasCancellationSource | undefined,
  request: (
    command: CanvasSurfaceCommand,
    cancellation?: CanvasCancellationSource,
  ) => Promise<CanvasMessage>,
): Promise<CanvasSurfaceResult> {
  if (cancellation?.cancelled())
    return { status: 'cancelled', message: 'Canvas surface request cancelled.' };
  const address = Object.freeze({ ...command.address });
  const revision = command.revision;
  const type = command.type;
  const admitted =
    command.type === 'surface.viewport.request'
      ? { ...command, address, command: Object.freeze({ ...command.command }) }
      : { ...command, address };
  const reply = await request(admitted, cancellation);
  if (reply.type === 'surface.viewport.reply') {
    if (
      type !== 'surface.viewport.request' ||
      reply.revision !== revision ||
      !sameSurface(reply.address, address)
    )
      throw new Error('Canvas viewport reply does not match its attachment.');
    return {
      status: reply.status,
      message: reply.message,
      viewport: snapshotCanvasSurfaceViewport(reply.viewport),
    };
  }
  if (
    reply.type !== 'surface.reply' ||
    reply.revision !== revision ||
    !sameSurface(reply.address, address) ||
    (reply.status === 'ready' && type !== 'surface.prepare') ||
    (reply.status === 'applied' &&
      (type === 'surface.prepare' || type === 'surface.viewport.request'))
  )
    throw new Error('Canvas surface reply does not match its attachment.');
  return { status: reply.status, message: reply.message };
}

/** Compare every component rather than treating a public attachment label as authority. */
function sameSurface(left: CanvasSurfaceAddress, right: CanvasSurfaceAddress): boolean {
  return (
    left.sessionId === right.sessionId &&
    left.viewId === right.viewId &&
    left.windowId === right.windowId &&
    left.attachmentId === right.attachmentId &&
    left.generation === right.generation
  );
}
