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
/** Provision workflow input surfaces on a canvas that owns their documents. */
import type { CanvasWorkflowCommand, CanvasWorkflowResult } from './CanvasProtocol.generated.js';
import type { CanvasCancellationSource } from './CanvasHost.js';

/**
 * Apply SugarCubes' provisioning decisions to the canvas's per-workflow input documents.
 *
 * `sync` replaces one workflow's surface set and maps every surface and mask key to the document
 * and layer identities the host assigned; surfaces it omits are retired. Other commands address
 * those keys. SugarCubes owns which surfaces exist; the host owns their documents and pixels.
 */
export interface CanvasWorkflowHost {
  readonly canvasId: string;
  execute(
    command: CanvasWorkflowCommand,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasWorkflowResult>;
}
