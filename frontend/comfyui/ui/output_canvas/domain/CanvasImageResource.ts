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
/** Define browser image admissions through opaque payload and handle ports. */

import type { ImageDimensions } from '../../execution/domain/WorkflowOutput.js';
import type { CanvasResourceStatus } from './CanvasViewPort.js';
import type { CanvasSelection } from './CanvasViewState.js';

export interface CanvasImageInput<Payload> {
  readonly key: string;
  /** Qualify inspection to the Run that owns these pixels, including outgoing replacement frames. */
  readonly runOrder: number;
  /** Correlate the actual displayed handle with its logical inspection slot during replacement. */
  readonly selection: CanvasSelection;
  readonly lane: string;
  readonly fallbackKey: string | null;
  readonly dimensions: ImageDimensions | null;
  readonly role: 'main' | 'thumbnail';
  readonly preview: boolean;
  readonly payload: Payload;
}

export interface CanvasDecodedImage<Handle> {
  readonly handle: Handle;
  readonly dimensions: ImageDimensions;
  /** Release the decoded handle and any owned URL; repeated release is harmless. */
  release(): void;
}

export interface CanvasImageLoad<Handle> {
  readonly completion: Promise<CanvasDecodedImage<Handle>>;
  /** Cancel transport and relinquish owned handles even if a platform decode settles later. */
  cancel(): void;
}

export interface CanvasImageLoadPort<Payload, Handle> {
  /** Start one admitted image without retaining its payload after handing it to the platform. */
  start(input: CanvasImageInput<Payload>): CanvasImageLoad<Handle>;
}

export interface CanvasImageFrame<Handle> {
  readonly key: string;
  readonly runOrder: number;
  readonly selection: CanvasSelection;
  readonly preview: boolean;
  readonly handle: Handle;
  readonly dimensions: ImageDimensions;
}

export interface CanvasImageSlotSnapshot<Handle> {
  readonly slot: string;
  readonly frame: CanvasImageFrame<Handle> | null;
  readonly status: CanvasResourceStatus;
}

export interface CanvasImageDiagnostics {
  readonly displayed: number;
  readonly pendingDecodes: number;
  readonly queuedInputs: number;
  readonly previewDrops: number;
  readonly estimatedBytes: number;
  readonly unknownLoads: number;
  readonly activeDeadlines: number;
}

/** Count estimated RGBA storage without claiming control over browser/GPU caches. */
export function canvasImageBytes(dimensions: ImageDimensions): number {
  if (
    ![dimensions.width, dimensions.height].every(
      (value) => Number.isSafeInteger(value) && value > 0,
    )
  )
    throw new RangeError('Canvas decoded dimensions are invalid.');
  const bytes = dimensions.width * dimensions.height * 4;
  if (!Number.isSafeInteger(bytes)) throw new RangeError('Canvas decoded estimate is invalid.');
  return bytes;
}
