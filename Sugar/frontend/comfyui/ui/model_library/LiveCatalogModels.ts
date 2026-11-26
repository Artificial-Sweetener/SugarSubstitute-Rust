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
/** Give an open surface the part of the catalog it shows, kept current while it is open. */
import type { LiveModels, ModelCatalogItem, ModelCatalogSnapshot } from './ModelCatalog.js';

/** The catalog owner as a surface following it needs it. */
export interface WatchedCatalog {
  watch(
    listener: (snapshot: ModelCatalogSnapshot) => void,
    reportError: (error: unknown) => void,
  ): () => void;
}

/**
 * Select models from the catalog and keep the selection current. A refreshed model, or an image
 * that arrives, reaches the surface without the surface being reopened.
 */
export function liveCatalogModels(
  catalog: WatchedCatalog,
  held: ModelCatalogSnapshot,
  select: (snapshot: ModelCatalogSnapshot) => readonly ModelCatalogItem[],
  reportError: (error: unknown) => void,
): LiveModels {
  let shown = select(held);
  return {
    current: () => shown,
    watch: (changed) =>
      catalog.watch((next) => {
        shown = select(next);
        changed();
      }, reportError),
  };
}
