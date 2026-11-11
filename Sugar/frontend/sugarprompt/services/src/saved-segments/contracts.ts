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
/** Stable host-owned associations keep display labels separate from storage identities. */
export type SavedSegmentScope =
  | {
      readonly kind: 'global';
      readonly provider: null;
      readonly key: string;
      readonly label: string;
    }
  | {
      readonly kind: 'family' | 'provider-model' | 'model-version' | 'local-model';
      readonly provider: string;
      readonly key: string;
      readonly label: string;
    };
/** Library contexts accept Global only; workflow scopes are supplied from host model metadata. */
export interface SavedSegmentContext {
  readonly mode: 'library' | 'workflow';
  readonly scopes: readonly SavedSegmentScope[];
}
/** Exact source is host data, including CR, LF, CRLF and astral UTF-16 characters. */
export interface SavedSegment {
  readonly id: string;
  readonly label: string;
  readonly text: string;
  readonly associations: readonly SavedSegmentScope[];
}
/** Saving adds one association; the host owns payload deduplication and shared label updates. */
export interface SavedSegmentSave {
  readonly label: string;
  readonly text: string;
  readonly scope: SavedSegmentScope;
}
/** Hosts decode their persisted schema before returning unknown data for runtime admission. */
export type SavedSegmentRead =
  | { readonly status: 'ready'; readonly segments: unknown }
  | { readonly status: 'unavailable' };
/** Abort must prevent subsequent persistence; storage migration and atomicity belong to the host. */
export interface SavedSegmentAdapter {
  read(context: SavedSegmentContext, signal: AbortSignal): Promise<SavedSegmentRead>;
  save(
    value: SavedSegmentSave,
    signal: AbortSignal,
  ): Promise<{ readonly status: 'saved' | 'unavailable' }>;
}
/** Prepared applicable scopes and groups are rebuilt off interaction and reused by menus. */
export interface SavedSegmentCatalog {
  readonly context: SavedSegmentContext;
  readonly groups: readonly {
    readonly scope: SavedSegmentScope;
    readonly segments: readonly SavedSegment[];
  }[];
}
