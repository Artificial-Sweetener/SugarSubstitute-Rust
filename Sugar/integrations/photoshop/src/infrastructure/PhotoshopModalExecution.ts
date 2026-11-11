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
/** Own bounded admission to Photoshop's exclusive native execution scope. */
import { core } from 'photoshop';

export type PhotoshopModalExecution = Parameters<Parameters<typeof core.executeAsModal>[0]>[0];
const MODAL_QUEUE_WAIT_SECONDS = 1;

/** Bound queued native operations using the host API's seconds unit without retaining a scope during generation. */
export function runPhotoshopModal(
  commandName: string,
  operation: (execution: PhotoshopModalExecution) => Promise<void>,
): Promise<void> {
  return core.executeAsModal(operation, { commandName, timeOut: MODAL_QUEUE_WAIT_SECONDS });
}
