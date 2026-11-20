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
/** Generated from file-choice.schema.json by scripts/integrations/generate-file-choice-contract.ts. */
/** Describe the schema-defined FileChoiceFailure wire value. */
export type FileChoiceFailure = 'busy' | 'unavailable';
/** Describe the schema-defined FileChoiceChosenMessage wire value. */
export type FileChoiceChosenMessage = {
  readonly protocol: 'sugarcubes.file-choice';
  readonly version: 1;
  readonly session: string;
  readonly type: 'chosen';
  readonly request: string;
  readonly name: string;
  readonly relative: string;
};
/** Describe the schema-defined FileChoiceDismissedMessage wire value. */
export type FileChoiceDismissedMessage = {
  readonly protocol: 'sugarcubes.file-choice';
  readonly version: 1;
  readonly session: string;
  readonly type: 'dismissed';
  readonly request: string;
};
/** Describe the schema-defined FileChoiceFailedMessage wire value. */
export type FileChoiceFailedMessage = {
  readonly protocol: 'sugarcubes.file-choice';
  readonly version: 1;
  readonly session: string;
  readonly type: 'failed';
  readonly request: string;
  readonly reason: FileChoiceFailure;
};
/** Describe the schema-defined FileChoiceOpenMessage wire value. */
export type FileChoiceOpenMessage = {
  readonly protocol: 'sugarcubes.file-choice';
  readonly version: 1;
  readonly session: string;
  readonly type: 'open';
  readonly request: string;
  readonly folder: string;
  readonly extension: string;
};
/** Describe the schema-defined FileChoiceSaveMessage wire value. */
export type FileChoiceSaveMessage = {
  readonly protocol: 'sugarcubes.file-choice';
  readonly version: 1;
  readonly session: string;
  readonly type: 'save';
  readonly request: string;
  readonly folder: string;
  readonly name: string;
  readonly extension: string;
};
/** Describe the schema-defined FileChoiceHostMessage wire value. */
export type FileChoiceHostMessage =
  | FileChoiceChosenMessage
  | FileChoiceDismissedMessage
  | FileChoiceFailedMessage;
/** Describe the schema-defined FileChoiceClientMessage wire value. */
export type FileChoiceClientMessage = FileChoiceOpenMessage | FileChoiceSaveMessage;
/** Describe every version-one file choice message. */
export type FileChoiceMessage =
  | FileChoiceChosenMessage
  | FileChoiceDismissedMessage
  | FileChoiceFailedMessage
  | FileChoiceOpenMessage
  | FileChoiceSaveMessage;
