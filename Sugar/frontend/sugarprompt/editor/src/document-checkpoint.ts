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
/** Public native document retention excludes configuration, rendering and external resources. */
export const documentCheckpointBrand: unique symbol = Symbol();
/** Opaque document identity is constructed only by the authenticated native lifetime owner. */
export interface DocumentCheckpoint {
  readonly [documentCheckpointBrand]: true;
  /** Read exact source derived from the retained immutable native document; released or disposed handles throw. */
  readonly source: string;
  /** Release this document's retained native fields once; later restoration rejects explicitly. */
  release(): void;
}

/** Finite measured capture never publishes a stale or unfinished composition document. */
export type DocumentCaptureOutcome =
  | { readonly status: 'captured'; readonly checkpoint: DocumentCheckpoint }
  | {
      readonly status: 'rejected';
      readonly reason: 'composing' | 'cancelled' | 'superseded' | 'disposed' | 'deadline';
    };

/** Authentication rejects before restoring any native state or provider lifetime. */
export type DocumentCheckpointRejection = 'foreign-checkpoint' | 'released-checkpoint' | 'disposed';
