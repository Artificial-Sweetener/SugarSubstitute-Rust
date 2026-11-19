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
/** Generated from file-drag.schema.json by scripts/integrations/generate-file-drag-contract.ts. */
/** Describe the schema-defined FileDragFile wire value. */
export type FileDragFile = { readonly name: string };
/** Describe the schema-defined FileDragEnteredMessage wire value. */
export type FileDragEnteredMessage = {
  readonly protocol: 'sugarcubes.file-drag';
  readonly version: 1;
  readonly session: string;
  readonly type: 'entered';
  readonly files: readonly FileDragFile[];
};
/** Describe the schema-defined FileDragLeftMessage wire value. */
export type FileDragLeftMessage = {
  readonly protocol: 'sugarcubes.file-drag';
  readonly version: 1;
  readonly session: string;
  readonly type: 'left';
};
/** Describe the schema-defined FileDragHostMessage wire value. */
export type FileDragHostMessage = FileDragEnteredMessage | FileDragLeftMessage;
/** Describe every version-one file drag message. */
export type FileDragMessage = FileDragEnteredMessage | FileDragLeftMessage;
