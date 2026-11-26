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
/** Stand in for the catalog behind a surface that follows its models while it is open. */
import type {
  LiveModels,
  ModelCatalogItem,
} from '../../../frontend/comfyui/ui/model_library/ModelCatalog.js';

export interface TestLiveModels {
  readonly models: LiveModels;
  /** Publish a new catalog generation to every surface still watching. */
  publish(next: readonly ModelCatalogItem[]): void;
  /** How many surfaces are watching. */
  watchers(): number;
}

/** Hold a list of models that a proof replaces the way a catalog change does. */
export function liveModels(initial: readonly ModelCatalogItem[]): TestLiveModels {
  let current = initial;
  const watching = new Set<() => void>();
  return {
    models: {
      current: () => current,
      watch: (changed) => {
        watching.add(changed);
        return () => watching.delete(changed);
      },
    },
    publish: (next) => {
      current = next;
      for (const changed of [...watching]) changed();
    },
    watchers: () => watching.size,
  };
}
