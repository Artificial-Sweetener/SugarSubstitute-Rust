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
/** Translate correlated geometry, editing and workflow commands into typed host results. */
import type { CanvasCancellationSource } from '../contracts/CanvasHost.js';
import type { CanvasGeometryHost } from '../contracts/CanvasGeometry.js';
import type { CanvasWorkflowHost } from '../contracts/CanvasWorkflowSurfaces.js';
import type {
  CanvasEditCommand,
  CanvasEditRequest,
  CanvasEditResult,
  CanvasGeometryRequest,
  CanvasMessage,
  CanvasWorkflowRequest,
} from '../contracts/CanvasProtocol.generated.js';
import {
  CANCELLED_EDIT,
  CANCELLED_GEOMETRY,
  CANCELLED_WORKFLOW,
} from './CanvasCancelledReplies.js';

/** Borrow the client's request correlation, deadlines and canvas identity. */
export interface CanvasCommandPort {
  identity(): string;
  request(
    request:
      | Pick<CanvasGeometryRequest, 'type' | 'command'>
      | Pick<CanvasEditRequest, 'type' | 'command'>
      | Pick<CanvasWorkflowRequest, 'type' | 'command'>,
    reply: 'geometry.reply' | 'edit.reply' | 'workflow.reply',
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasMessage>;
}

/** Expose geometry reads and native sizing; an already cancelled caller sends nothing. */
export function canvasGeometryCommands(port: CanvasCommandPort): CanvasGeometryHost {
  return {
    get canvasId() {
      return port.identity();
    },
    async execute(command, cancellation) {
      if (cancellation?.cancelled()) return CANCELLED_GEOMETRY;
      const reply = await port.request(
        { type: 'geometry.request', command },
        'geometry.reply',
        cancellation,
      );
      if (reply.type !== 'geometry.reply') throw new Error('Invalid canvas geometry reply.');
      return reply.result;
    },
  };
}

/** Expose workflow surface provisioning; a timed-out command is never replayed. */
export function canvasWorkflowCommands(port: CanvasCommandPort): CanvasWorkflowHost {
  return {
    get canvasId() {
      return port.identity();
    },
    async execute(command, cancellation) {
      if (cancellation?.cancelled()) return CANCELLED_WORKFLOW;
      const reply = await port.request(
        { type: 'workflow.request', command },
        'workflow.reply',
        cancellation,
      );
      if (reply.type !== 'workflow.reply') throw new Error('Invalid canvas workflow reply.');
      return reply.result;
    },
  };
}

/** Execute editing commands; pushed state observation remains with the edit connection. */
export function canvasEditCommands(
  port: CanvasCommandPort,
): (
  command: CanvasEditCommand,
  cancellation?: CanvasCancellationSource,
) => Promise<CanvasEditResult> {
  return async (command, cancellation) => {
    if (cancellation?.cancelled()) return CANCELLED_EDIT;
    const reply = await port.request({ type: 'edit.request', command }, 'edit.reply', cancellation);
    if (reply.type !== 'edit.reply') throw new Error('Invalid canvas edit reply.');
    return reply.result;
  };
}
