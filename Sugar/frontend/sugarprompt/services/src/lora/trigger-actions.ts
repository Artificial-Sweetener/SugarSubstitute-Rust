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
/** Canonical whole-entry trigger action projection has one shared owner for inventory and effective-scene menus. */
import type { EffectiveLoraModelSnapshot, ScheduledLora } from './scheduled-contracts.js';
import { loraNameKey } from './names.js';

/** Prepare only executable nonblank configured entries, indexed by normalized physical backend identity. */
export function prepareLoraTriggerActions(
  snapshot: EffectiveLoraModelSnapshot,
): ReadonlyMap<string, ScheduledLora> {
  return new Map(
    snapshot.status === 'ready'
      ? snapshot.models
          .filter((model) => model.insertionText)
          .map((model) => [loraNameKey(model.model.backendValue), model])
      : [],
  );
}
