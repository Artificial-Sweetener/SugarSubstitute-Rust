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
/** A retired decode settles its owner immediately and releases late pixels even when the decoder ignores abort. */
import type { LoraMediaDecoder, LoraMediaReference, LoraRaster } from './lora-media-contracts.js';

/** Unexpected current failure retains its cause; retirement never becomes a successful image. */
export type LoraDecodeOutcome =
  | { readonly status: 'ready'; readonly raster: LoraRaster | null }
  | { readonly status: 'failed'; readonly cause: unknown };

/** Promise continuations retain only this detached job after cancellation, never a view, cache or observer. */
export class LoraMediaJob {
  private readonly cancellation = new AbortController();
  private completion: ((outcome: LoraDecodeOutcome) => void) | undefined;
  /** The owner installs the job before starting a possibly synchronous/reentrant decoder. */
  constructor(completion: (outcome: LoraDecodeOutcome) => void) {
    this.completion = completion;
  }
  /** Complete once; late ready values are disposed even after an ignored abort. */
  start(reference: LoraMediaReference, decode: LoraMediaDecoder): void {
    try {
      void decode(reference, this.cancellation.signal).then(
        (raster) => {
          const complete = this.completion;
          this.completion = undefined;
          if (complete) complete({ status: 'ready', raster });
          else raster?.dispose();
        },
        (cause: unknown) => {
          const complete = this.completion;
          this.completion = undefined;
          complete?.({ status: 'failed', cause });
        },
      );
    } catch (cause) {
      const complete = this.completion;
      this.completion = undefined;
      complete?.({ status: 'failed', cause });
    }
  }
  /** Release the observer before host abort callbacks; expected retirement has no publication. */
  cancel(): void {
    this.completion = undefined;
    this.cancellation.abort();
  }
}
