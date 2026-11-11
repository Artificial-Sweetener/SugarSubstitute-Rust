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
/** Sequence bounded image preparation and one native document transaction through explicit ports. */
import type { CanvasCancellationSource } from '../../../contracts/CanvasHost.js';
import type {
  CanvasInsertImage,
  CanvasEncodedImage,
  CanvasInsertResult,
} from '../../../contracts/CanvasProtocol.generated.js';
/** Identify a prepared image without leaking files or native editor values into application code. */
export interface PreparedCanvasImage {
  readonly key: string;
  readonly width: number;
  readonly height: number;
  dispose(): Promise<void>;
}
/** Supply native materialization, transaction ownership and actionable cleanup diagnostics. */
export interface CanvasInsertionPort {
  prepare(
    image: CanvasEncodedImage,
    cancellation: CanvasCancellationSource,
  ): Promise<PreparedCanvasImage>;
  insert(
    request: CanvasInsertImage,
    image: PreparedCanvasImage,
    cancellation: CanvasCancellationSource,
  ): Promise<CanvasInsertResult>;
  report(error: unknown): void;
}
/** Keep network preparation outside the native document mutation lifetime. */
export class InsertCanvasImage {
  /** Bind native collaborators while retaining sequencing in the application owner. */
  constructor(private readonly port: CanvasInsertionPort) {}
  /** Commit the captured image once and release its transient material even when the transaction fails. */
  async execute(
    request: CanvasInsertImage,
    cancellation: CanvasCancellationSource,
  ): Promise<CanvasInsertResult> {
    if (cancellation.cancelled())
      return { status: 'cancelled', message: 'Canvas insertion cancelled.' };
    const image = await this.port.prepare(request.image, cancellation);
    try {
      return cancellation.cancelled()
        ? { status: 'cancelled', message: 'Canvas insertion cancelled.' }
        : await this.port.insert(request, image, cancellation);
    } finally {
      try {
        await image.dispose();
      } catch (error: unknown) {
        this.port.report(error);
      }
    }
  }
}
