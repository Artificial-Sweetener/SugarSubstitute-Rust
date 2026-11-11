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
/** Run the shared mask preparation policy without coupling callers to worker or editor APIs. */
import type { CanvasPixelRaster } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type { MaskEditSettings } from '../domain/MaskEditSettings.js';
import type { EditedMaskInput } from '../domain/MaskInputEditing.js';

/** Distinguish replaceable preview work from immutable submitted generation inputs. */
export interface MaskEditingPort {
  prepare(
    source: CanvasPixelRaster,
    settings: MaskEditSettings,
    purpose: 'preview' | 'submission',
  ): Promise<EditedMaskInput>;
}
