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
/** Drive document editing tools independently of editor identity and presentation. */
import type {
  CanvasEditCommand,
  CanvasEditResult,
  CanvasEditState,
} from './CanvasProtocol.generated.js';
import type { CanvasCancellationSource } from './CanvasHost.js';
import type { CanvasChangeObservation } from './CanvasChangeObservation.js';

/**
 * Execute editing commands against the canvas's active document.
 *
 * A successful command resolves with the resulting state. The host also pushes every state change
 * without a watch request, so observation is ready as soon as a listener subscribes; each pushed
 * state carries a revision that only increases within one canvas session. Selection bounds are in
 * surface CSS pixels and follow pan and zoom.
 */
export interface CanvasEditHost {
  readonly canvasId: string;
  execute(
    command: CanvasEditCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasEditResult>;
  readonly changes: CanvasChangeObservation<CanvasEditState>;
}
