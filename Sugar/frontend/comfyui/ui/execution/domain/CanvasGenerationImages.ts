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
/** Prepare generated image bytes through a host-independent, cancellable media boundary. */
import type {
  CanvasCancellationSource,
  CanvasEncodedImage,
} from '../../core/embedded/CanvasRuntime.js';
import type { OutputFileReference } from './WorkflowOutput.js';
import type { CanvasImageRegion } from '../../../../../integrations/contracts/CanvasImageRegion.js';
import type { CanvasImagePosition } from '../../../../../integrations/contracts/CanvasProtocol.generated.js';

/** Materialize a normal image or a transparent region without leaking browser raster objects. */
export interface PreparedGenerationImage {
  readonly image: CanvasEncodedImage;
  readonly position: CanvasImagePosition | null;
}

export interface CanvasGenerationImages {
  load(
    file: OutputFileReference,
    cancellation: CanvasCancellationSource,
    region: CanvasImageRegion | null,
  ): Promise<PreparedGenerationImage | null>;
}
