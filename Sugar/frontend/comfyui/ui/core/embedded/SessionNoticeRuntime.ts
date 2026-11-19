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
/** Bundle the portable session notice contract, both released versions, for browser consumers. */
export { isSessionNoticeHostMessage } from '../../../../../integrations/shared/SessionNoticeValidation.generated.js';
export type {
  SessionNoticeClientMessage,
  SessionNoticeHostMessage,
} from '../../../../../integrations/contracts/SessionNoticeProtocol.generated.js';
export { isSessionNoticeHostMessage as isLinedSessionNoticeHostMessage } from '../../../../../integrations/shared/SessionNoticeV2Validation.generated.js';
export type {
  SessionNoticeClientMessage as LinedSessionNoticeClientMessage,
  SessionNoticeLineMessage,
  SessionNoticeTone,
} from '../../../../../integrations/contracts/SessionNoticeV2Protocol.generated.js';
