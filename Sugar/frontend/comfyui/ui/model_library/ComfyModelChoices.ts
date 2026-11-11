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
/** Refresh native Comfy choices after the server publishes a verified artifact. */
import { isRecord } from '../types/common.js';

/** Use the host's model-option refresh lifecycle without changing any selected field values. */
export async function refreshComfyModelChoices(app: unknown): Promise<void> {
  if (!isRecord(app) || typeof app.refreshComboInNodes !== 'function')
    throw new Error('The model was downloaded. Reload ComfyUI to refresh available model choices.');
  await app.refreshComboInNodes.call(app);
}
