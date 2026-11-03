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
/** Declare executable canvas capabilities independently of execution engines and editor APIs. */
import type { CanvasCancellationSource, CanvasLayerHost } from './CanvasHost.js';
import type { CanvasMaskHost } from './CanvasMasks.js';
import type { CanvasSourceHost } from './CanvasSources.js';
import type { CanvasGeometryHost } from './CanvasGeometry.js';
import type { CanvasSurfaceHost } from './CanvasSurface.js';
import type { CanvasEditHost } from './CanvasEditing.js';
import type { CanvasWorkflowHost } from './CanvasWorkflowSurfaces.js';

/** Receive supported output in a destination captured before execution begins. */
export interface CanvasOutputBinding<Output> {
  receive(output: Output, cancellation: CanvasCancellationSource): void | Promise<void>;
}

/** Preserve synchronous view updates while document adapters request serialized asynchronous operations. */
export type CanvasOutputCapability<Submission, Output> =
  | {
      readonly ordering: 'immediate';
      bind(
        submission: Submission,
        cancellation: CanvasCancellationSource,
      ): {
        receive(output: Output, cancellation: CanvasCancellationSource): void;
      } | null;
    }
  | {
      readonly ordering: 'serial';
      bind(
        submission: Submission,
        cancellation: CanvasCancellationSource,
      ): CanvasOutputBinding<Output> | null | Promise<CanvasOutputBinding<Output> | null>;
    };

/** Describe peer canvases through operations they actually implement. */
export interface CanvasAdapter<Submission, Output, ViewState, Preview> {
  readonly id: string;
  readonly capabilities: {
    readonly geometry?: CanvasGeometryHost | undefined;
    readonly layers?: CanvasLayerHost | undefined;
    readonly masks?: CanvasMaskHost | undefined;
    readonly sources?: CanvasSourceHost | undefined;
    readonly editing?: CanvasEditHost | undefined;
    readonly workflow?: CanvasWorkflowHost | undefined;
    readonly surface?: CanvasSurfaceHost | undefined;
    readonly output: CanvasOutputCapability<Submission, Output>;
    readonly outputView?: {
      update(state: ViewState): void;
      restore(state: ViewState): void;
    };
    readonly preview?: {
      wants(preview: Preview): boolean;
      receive(preview: Preview): void;
    };
  };
}
