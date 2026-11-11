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
/** Describe the typed outcome a caller receives after cancelling before the host answers. */
import type {
  CanvasEditResult,
  CanvasGeometryResult,
  CanvasInsertResult,
  CanvasMessage,
  CanvasWorkflowResult,
} from '../contracts/CanvasProtocol.generated.js';

/** Report an insertion the caller cancelled before the host answered. */
export const CANCELLED_INSERT: CanvasInsertResult = Object.freeze({
  status: 'cancelled',
  message: 'Canvas insertion cancelled.',
});
/** Report a geometry command the caller cancelled before the host answered. */
export const CANCELLED_GEOMETRY: CanvasGeometryResult = Object.freeze({
  kind: 'failure',
  status: 'cancelled',
  message: 'Canvas sizing cancelled.',
});
/** Report an editing command the caller cancelled before the host answered. */
export const CANCELLED_EDIT: CanvasEditResult = Object.freeze({
  kind: 'failure',
  status: 'cancelled',
  message: 'Canvas edit cancelled.',
});
/** Report a workflow command the caller cancelled before the host answered. */
export const CANCELLED_WORKFLOW: CanvasWorkflowResult = Object.freeze({
  kind: 'failure',
  status: 'cancelled',
  message: 'Canvas workflow request cancelled.',
});

/**
 * Answer a caller-cancelled request locally with the reply its own type expects.
 *
 * Only insertion, geometry, editing and workflow requests accept caller cancellation; the host
 * still receives a cancel message, and any later host reply is refused as uncorrelated.
 */
export function cancelledCanvasReply(
  request: 'image.insert' | 'geometry.request' | 'edit.request' | 'workflow.request',
  session: string,
  requestId: number,
): CanvasMessage {
  const envelope = { protocol: 'sugarcubes.canvas', version: 3, session, requestId } as const;
  if (request === 'geometry.request')
    return { ...envelope, type: 'geometry.reply', result: CANCELLED_GEOMETRY };
  if (request === 'edit.request')
    return { ...envelope, type: 'edit.reply', result: CANCELLED_EDIT };
  if (request === 'workflow.request')
    return { ...envelope, type: 'workflow.reply', result: CANCELLED_WORKFLOW };
  return { ...envelope, type: 'image.reply', result: CANCELLED_INSERT };
}
