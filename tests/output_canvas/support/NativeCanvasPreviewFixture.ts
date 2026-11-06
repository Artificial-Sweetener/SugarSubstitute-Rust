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
/** Control only external native acknowledgements, observation and immutable Blob reads. */
import { jest } from '@jest/globals';
import type {
  CanvasSurfaceCommand,
  CanvasSurfaceHost,
  CanvasSurfaceResult,
  CanvasSurfacePresentationChange,
  CanvasSurfaceViewportState,
} from '../../../integrations/contracts/CanvasSurface.js';
import type { CanvasObservationStatus } from '../../../integrations/contracts/CanvasChangeObservation.js';
import type { CanvasSurfaceGeometry } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasViewPort.js';

export const previewGeometry: CanvasSurfaceGeometry = {
  coordinateSpace: 'client',
  units: 'css-pixels',
  origin: { x: 0, y: 0 },
  viewport: { width: 500, height: 400 },
  clip: { x: 0, y: 0, width: 500, height: 400 },
  physicalPixelsPerCssPixel: 1,
  visible: true,
  focused: false,
};
export const finalViewport: CanvasSurfaceViewportState = {
  mode: 'native',
  physicalScale: 1,
  dimensions: { width: 1024, height: 768 },
};
export const previewViewport: CanvasSurfaceViewportState = {
  mode: 'fit',
  physicalScale: 2,
  dimensions: { width: 19, height: 17 },
};

/** Hold each native ticket until the real presentation owner observes its terminal result. */
export function nativePreviewHost(preview = true) {
  const calls: CanvasSurfaceCommand[] = [];
  const pending: {
    resolve(result: CanvasSurfaceResult): void;
    cancelled: boolean;
    settled: boolean;
  }[] = [];
  const listeners = new Set<(change: CanvasSurfacePresentationChange) => void>();
  let ready!: (status: CanvasObservationStatus) => void;
  const observing = new Promise<CanvasObservationStatus>((resolve) => {
    ready = resolve;
  });
  const stop = jest.fn();
  const host: CanvasSurfaceHost = {
    capabilities: {
      schema: 2,
      imageFormats: ['png'],
      previewFormats: preview ? ['jpeg'] : [],
      viewportCommands: ['fit', 'native', 'zoom', 'pan'],
      viewportState: true,
      comparison: false,
      overview: false,
      input: false,
    },
    presentation: {
      subscribe: (listener) => {
        listeners.add(listener);
        return {
          ready: observing,
          stop: () => {
            stop();
            listeners.delete(listener);
          },
        };
      },
    },
    execute: (command, cancellation) => {
      calls.push(command);
      return new Promise<CanvasSurfaceResult>((resolve) => {
        const record = {
          cancelled: false,
          settled: false,
          resolve: (result: CanvasSurfaceResult) => {
            record.settled = true;
            unsubscribe?.();
            resolve(result);
          },
        };
        const unsubscribe = cancellation?.subscribe(() => {
          record.cancelled = true;
        });
        pending.push(record);
      });
    },
  };
  return {
    host,
    calls,
    pending,
    ready,
    stop,
    listeners,
    emit: (change: CanvasSurfacePresentationChange) => {
      for (const listener of listeners) listener(change);
    },
    respond: (
      index: number,
      status: CanvasSurfaceResult['status'] = 'applied',
      viewport?: CanvasSurfaceViewportState,
    ) => {
      pending[index]!.resolve(
        viewport ? { status, message: '', viewport } : { status, message: '' },
      );
    },
  };
}

/** Hold only bounded byte extraction, independently of native ticket ownership and decoding. */
export class NativePreviewBlob extends Blob {
  readonly reads: {
    start: number;
    end: number;
    resolve(value: ArrayBuffer): void;
    reject(error: Error): void;
  }[] = [];
  readonly encoded: Uint8Array;
  /** Retain small deterministic encoded bytes without invoking any image codec. */
  constructor(size = 3) {
    const bytes = new Uint8Array(size);
    for (let index = 0; index < size; index++) bytes[index] = index % 256;
    super([bytes], { type: 'image/jpeg' });
    this.encoded = bytes;
  }
  /** Expose a controllable external Blob extraction boundary for each bounded slice. */
  override slice(start = 0, end = this.size, contentType?: string): Blob {
    const slice = super.slice(start, end, contentType);
    slice.arrayBuffer = () =>
      new Promise<ArrayBuffer>((resolve, reject) => {
        this.reads.push({ start, end, resolve, reject });
      });
    return slice;
  }
  /** Complete one exact slice with the immutable bytes requested by the production owner. */
  release(index = 0): void {
    const read = this.reads[index];
    if (!read) throw new Error('Preview byte read did not start.');
    const bytes = new Uint8Array(read.end - read.start);
    bytes.set(this.encoded.subarray(read.start, read.end));
    read.resolve(bytes.buffer);
  }
}

/** Drain actual promise continuations; every external completion remains explicitly controlled. */
export async function settleNativePreview(): Promise<void> {
  for (let turn = 0; turn < 24; turn++) await Promise.resolve();
}
