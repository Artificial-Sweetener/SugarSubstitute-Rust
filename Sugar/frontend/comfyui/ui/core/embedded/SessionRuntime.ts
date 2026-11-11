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
/** Publish canonical portable session validation as one browser runtime beside the canvas and panel contracts. */
export {
  SESSION_DOCUMENT_BYTES,
  sessionGraph,
  sessionJsonScalars,
  sessionUuid,
  sessionScope,
  utf8Bytes,
} from '../../../../../integrations/shared/SessionPrimitives.js';
export {
  SESSION_CHUNK_BYTES,
  sameSessionFence,
  sessionAdmission,
  sessionMessageError,
} from '../../../../../integrations/shared/SessionProtocol.js';
export type { SessionErrorCode } from '../../../../../integrations/shared/SessionProtocol.js';
export {
  desktopCheckpointError,
  isDesktopCheckpoint,
} from '../../../../../integrations/shared/SessionSnapshot.js';
export {
  parseSessionJson,
  decodeDesktopCheckpoint,
} from '../../../../../integrations/shared/SessionDecode.js';
export {
  isSessionBinding,
  isSessionMessage,
} from '../../../../../integrations/shared/SessionValidation.generated.js';
export { isSessionScope } from '../../../../../integrations/shared/SessionOutputValidation.generated.js';
export { isSessionDocument } from '../../../../../integrations/shared/SessionSnapshotValidation.generated.js';
