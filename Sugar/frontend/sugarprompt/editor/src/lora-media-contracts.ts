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
/** Browser media preparation consumes validated host references without acquiring model metadata or owning source. */
import type { LoraThumbnail } from '../../services/src/index.js';

/** The immutable host descriptor is the complete catalog/media identity, reused by the metadata owner. */
export type LoraMediaReference = Extract<LoraThumbnail, { status: 'ready' }>;

/** A finite decoded raster paints into newly constructed widget canvases; only its preparation owner disposes it. */
export interface LoraRaster {
  readonly width: number;
  readonly height: number;
  /** Paint prepared pixels into a newly created canvas without decoding, I/O or managed-content mutation. */
  paint(canvas: HTMLCanvasElement): void;
  /** Release native pixels once when the preparation lifetime retires them. */
  dispose(): void;
}

/** Decode failure retains the fallback chip and differs from unavailable catalog authority. */
export type PreparedLoraMedia =
  | { readonly status: 'ready'; readonly raster: LoraRaster }
  | { readonly status: 'pending' }
  | { readonly status: 'unavailable' };

/** External browser decoding is cancellable; a null result is expected unavailable media, while unexpected errors retain their cause. */
export type LoraMediaDecoder = (
  reference: LoraMediaReference,
  signal: AbortSignal,
) => Promise<LoraRaster | null>;

/** Prepared defaults have no request, image, source or history authority. */
export const pendingLoraMedia: PreparedLoraMedia = Object.freeze({
  status: 'pending',
});
/** Every no-thumbnail and failed-decoding path uses the same explicit fallback state. */
export const unavailableLoraMedia: PreparedLoraMedia = Object.freeze({
  status: 'unavailable',
});
