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
/** Control the platform decode boundary while leaving browser canvas policies and DOM rendering real. */

import type { ImageDimensions } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type {
  CanvasDecodedImage,
  CanvasImageInput,
  CanvasImageLoad,
  CanvasImageLoadPort,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasImageResource.js';
import type { BrowserCanvasImagePayload } from '../../../frontend/comfyui/ui/output_canvas/infrastructure/BrowserCanvasImageLoader.js';
import { settleCanvasImages } from './CanvasImageFixture.js';

interface PlatformRequest {
  readonly input: CanvasImageInput<BrowserCanvasImagePayload>;
  readonly resolve: (image: CanvasDecodedImage<HTMLImageElement>) => void;
  readonly reject: (error: unknown) => void;
  released: boolean;
}

/** Resolve admitted requests explicitly and observe late completion after cancellation. */
export class BrowserCanvasPlatformFixture
  implements CanvasImageLoadPort<BrowserCanvasImagePayload, HTMLImageElement>
{
  readonly requests: PlatformRequest[] = [];
  disposed = false;

  /** Model only the platform's cancellable decode lease. */
  start(input: CanvasImageInput<BrowserCanvasImagePayload>): CanvasImageLoad<HTMLImageElement> {
    let resolve: PlatformRequest['resolve'] = () => {
      throw new Error('Platform request is uninitialized.');
    };
    let reject: PlatformRequest['reject'] = () => {
      throw new Error('Platform request is uninitialized.');
    };
    const completion = new Promise<CanvasDecodedImage<HTMLImageElement>>((ready, failed) => {
      resolve = ready;
      reject = failed;
    });
    const request = { input, resolve, reject, released: false };
    this.requests.push(request);
    return {
      completion,
      cancel: () => {
        request.released = true;
      },
    };
  }

  /** Finish even canceled platform work so the real scheduler proves late handle release. */
  async ready(
    index: number,
    dimensions: ImageDimensions = { width: 3840, height: 2160 },
  ): Promise<HTMLImageElement> {
    const request = this.requests[index];
    if (!request) throw new Error('Platform fixture request is missing.');
    const handle = document.createElement('img');
    handle.draggable = false;
    if (request.input.payload.kind === 'file') handle.src = request.input.payload.url;
    request.resolve({
      handle,
      dimensions,
      release: () => {
        request.released = true;
        handle.remove();
      },
    });
    await settleCanvasImages();
    return handle;
  }

  /** Report a real asynchronous platform failure through the owning scheduler. */
  async fail(index: number): Promise<void> {
    const request = this.requests[index];
    if (!request) throw new Error('Platform fixture request is missing.');
    request.reject(new Error('Platform decode failed.'));
    await settleCanvasImages();
  }

  /** Dispose the explicit loader boundary after all owned leases have been canceled. */
  dispose(): void {
    this.disposed = true;
  }
}
