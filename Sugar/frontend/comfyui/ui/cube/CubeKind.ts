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
/** Recognize a SugarCube by its durable kind marker, for every reader of graphs and documents. */

/** The `sugarcubes_kind` values a Cube carries: a persisted Cube or a workflow-only draft. */
export type CubeKind = 'cube' | 'cube_draft';

/** Whether a `sugarcubes_kind` value marks a Cube, persisted or draft. */
export function isCubeKind(value: unknown): value is CubeKind {
  return value === 'cube' || value === 'cube_draft';
}
