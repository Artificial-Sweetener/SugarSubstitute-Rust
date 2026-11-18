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
/** Word the notice that the previous close did not keep the person's latest work. */

/** What the previous close lost; at least one is true. */
export interface UnsavedWork {
  readonly workspace: boolean;
  readonly inputs: boolean;
}

/** A notice's heading and its one sentence. */
export interface UnsavedWorkWords {
  readonly title: string;
  readonly message: string;
}

/** Name exactly what was lost, so the person knows where to look. */
export function unsavedWorkWords(lost: UnsavedWork): UnsavedWorkWords {
  const what =
    lost.workspace && lost.inputs
      ? 'workflow changes and input canvas edits'
      : lost.inputs
        ? 'input canvas edits'
        : 'workflow changes';
  return {
    title: 'Latest changes not saved',
    message: `Your latest ${what} were not saved the last time the application closed.`,
  };
}
