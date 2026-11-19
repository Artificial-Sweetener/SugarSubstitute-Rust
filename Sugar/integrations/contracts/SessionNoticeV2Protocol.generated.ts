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
/** Generated from session-notice-v2.schema.json by scripts/integrations/generate-session-notice-contract.ts. */
/** Describe the schema-defined SessionNoticeTone wire value. */
export type SessionNoticeTone = 'info' | 'warn' | 'error';
/** Describe the schema-defined SessionNoticeNotSavedMessage wire value. */
export type SessionNoticeNotSavedMessage = {
  readonly protocol: 'sugarcubes.session-notice';
  readonly version: 2;
  readonly session: string;
  readonly type: 'not-saved';
  readonly notice: number;
  readonly workspace: boolean;
  readonly inputs: boolean;
};
/** Describe the schema-defined SessionNoticeLineMessage wire value. */
export type SessionNoticeLineMessage = {
  readonly protocol: 'sugarcubes.session-notice';
  readonly version: 2;
  readonly session: string;
  readonly type: 'line';
  readonly notice: number;
  readonly tone: SessionNoticeTone;
  readonly text: string;
  readonly lasting: boolean;
};
/** Describe the schema-defined SessionNoticeNoneMessage wire value. */
export type SessionNoticeNoneMessage = {
  readonly protocol: 'sugarcubes.session-notice';
  readonly version: 2;
  readonly session: string;
  readonly type: 'none';
};
/** Describe the schema-defined SessionNoticeWatchMessage wire value. */
export type SessionNoticeWatchMessage = {
  readonly protocol: 'sugarcubes.session-notice';
  readonly version: 2;
  readonly session: string;
  readonly type: 'watch';
};
/** Describe the schema-defined SessionNoticeShownMessage wire value. */
export type SessionNoticeShownMessage = {
  readonly protocol: 'sugarcubes.session-notice';
  readonly version: 2;
  readonly session: string;
  readonly type: 'shown';
  readonly notice: number;
};
/** Describe the schema-defined SessionNoticeHostMessage wire value. */
export type SessionNoticeHostMessage =
  | SessionNoticeNotSavedMessage
  | SessionNoticeLineMessage
  | SessionNoticeNoneMessage;
/** Describe the schema-defined SessionNoticeClientMessage wire value. */
export type SessionNoticeClientMessage = SessionNoticeWatchMessage | SessionNoticeShownMessage;
/** Describe every version-two session notice message. */
export type SessionNoticeMessage =
  | SessionNoticeNotSavedMessage
  | SessionNoticeLineMessage
  | SessionNoticeNoneMessage
  | SessionNoticeWatchMessage
  | SessionNoticeShownMessage;
