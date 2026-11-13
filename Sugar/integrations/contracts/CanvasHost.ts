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
/** Describe supported canvas implementations independently of a renderer or editor API. */
import type {
  CanvasContext,
  CanvasDescriptor,
  CanvasInsertImage,
  CanvasInsertResult,
} from './CanvasProtocol.generated.js';
import type { CanvasMaskHost } from './CanvasMasks.js';
import type { CanvasSourceHost } from './CanvasSources.js';
import type { CanvasGeometryHost } from './CanvasGeometry.js';
import type { CanvasSurfaceHost } from './CanvasSurface.js';
import type { CanvasEditHost } from './CanvasEditing.js';
import type { CanvasWorkflowHost } from './CanvasWorkflowSurfaces.js';
/** Expose common discovery without requiring document editing from display-only canvases. */
export interface CanvasHost {
  readCanvas(): CanvasDescriptor | null;
  /** Optional renderer-owned output surface, negotiated independently of layer editing. */
  readonly surface?: CanvasSurfaceHost | null;
}
/** Supply image insertion only on hosts declaring the corresponding capability. */
export interface CanvasLayerHost extends CanvasHost {
  readonly geometry?: CanvasGeometryHost | null;
  readonly masks?: CanvasMaskHost | null;
  readonly sources?: CanvasSourceHost | null;
  /** Editing commands; exposed to consumers only when the descriptor advertises editing. */
  readonly editing?: CanvasEditHost | null;
  /** Workflow surface provisioning; exposed only when the descriptor advertises it. */
  readonly workflow?: CanvasWorkflowHost | null;
  readContext(): Promise<CanvasContext>;
  insertImage(
    image: CanvasInsertImage,
    cancellation?: CanvasCancellationSource,
  ): Promise<CanvasInsertResult>;
}
/** Describe cancellation without browser AbortSignal or native editor dependencies. */
export interface CanvasCancellationSource {
  cancelled(): boolean;
  subscribe(cancelled: () => void): () => void;
}
/** Publish the existing web canvas's capabilities without adding document or layer behavior. */
export const WEB_CANVAS: CanvasDescriptor = Object.freeze({
  id: 'web',
  label: 'SugarCubes Web',
  capabilities: Object.freeze({
    displayOutput: true,
    insertImageLayer: false,
    positionImageLayer: false,
  }),
});
