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
/** Expose optional mask editing independently of editor identity and output presentation. */
import type {
  CanvasMaskQuery,
  CanvasMaskResult,
  CanvasMaskChanged,
} from './CanvasMasksProtocol.generated.js';
import type { CanvasChangeObservation } from './CanvasChangeObservation.js';

/** Supply committed coverage changes and coherent capture through one canvas capability. */
export interface CanvasMaskHost {
  readonly canvasId: string;
  /** Capture all requested masks under one coherent document revision; return explicit unsupported failures. */
  execute(command: CanvasMaskQuery): Promise<CanvasMaskResult>;
  readonly changes: CanvasChangeObservation<CanvasMaskChanged> | null;
}
