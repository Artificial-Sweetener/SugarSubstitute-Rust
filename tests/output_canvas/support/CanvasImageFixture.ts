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
/** Control image loading, cancellation and owned-handle release without browser caches. */

import type { ImageDimensions } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type {
  CanvasDecodedImage,
  CanvasImageInput,
  CanvasImageLoad,
  CanvasImageLoadPort,
} from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasImageResource.js';
import { CanvasImageBudget } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageBudget.js';
import { CanvasImageDecodeScheduler } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageDecodeScheduler.js';
import { CanvasImageResources } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasImageResources.js';
import { WorkflowTestClock } from '../../execution/support/workflow_results.js';

export interface CanvasImageHandle {
  readonly key: string;
}
interface ControlledLoad {
  readonly key: string;
  readonly preview: boolean;
  cancelled: boolean;
  released: boolean;
  readonly resolve: (image: CanvasDecodedImage<CanvasImageHandle>) => void;
  readonly reject: (error: unknown) => void;
}

/** Keep diagnostic request history in the test fixture while production retains only active slots. */
export class CanvasImagePortFixture implements CanvasImageLoadPort<string, CanvasImageHandle> {
  readonly requests: ControlledLoad[] = [];
  failStart = false;

  /** Start a controllable request, retaining no runtime browser or file dependency. */
  start(input: CanvasImageInput<string>): CanvasImageLoad<CanvasImageHandle> {
    if (this.failStart) throw new Error('Platform start failed');
    let resolve: ControlledLoad['resolve'] = () => {
      throw new Error('Decode resolver is uninitialized.');
    };
    let reject: ControlledLoad['reject'] = () => {
      throw new Error('Decode rejecter is uninitialized.');
    };
    const completion = new Promise<CanvasDecodedImage<CanvasImageHandle>>((ready, failed) => {
      resolve = ready;
      reject = failed;
    });
    const request = {
      key: input.key,
      preview: input.preview,
      cancelled: false,
      released: false,
      resolve,
      reject,
    };
    this.requests.push(request);
    return {
      completion,
      cancel: () => {
        request.cancelled = true;
        request.released = true;
      },
    };
  }

  /** Resolve even a canceled request to exercise noncancelable platform completion. */
  ready(index: number, dimensions: ImageDimensions = { width: 3840, height: 2160 }): void {
    const request = this.#require(index);
    request.resolve({
      handle: Object.freeze({ key: request.key }),
      dimensions: Object.freeze({ ...dimensions }),
      release: () => {
        request.released = true;
      },
    });
  }

  /** Reject a request with observable original exception context. */
  fail(index: number): void {
    this.#require(index).reject(new Error('Decode failed'));
  }

  /** Count owned handles rather than pretending to model a browser's internal cache. */
  get allocations(): number {
    return this.requests.filter((request) => !request.released).length;
  }

  /** Count preview URL-like ownership held by active fake requests. */
  get urls(): number {
    return this.requests.filter((request) => request.preview && !request.released).length;
  }

  /** Require an existing admission for deterministic fixture failures. */
  #require(index: number): ControlledLoad {
    const request = this.requests[index];
    if (!request) throw new Error('Image fixture admission is missing.');
    return request;
  }
}

/** Create a complete visible input with stable per-run preview lane identity. */
export function canvasImageInput(
  key: string,
  preview = false,
  dimensions: ImageDimensions | null = { width: 3840, height: 2160 },
): CanvasImageInput<string> {
  return {
    key,
    lane: preview ? 'preview:current-run' : key,
    fallbackKey: null,
    preview,
    dimensions,
    role: 'main',
    payload: key,
  };
}

/** Compose real resource policies around controllable platform and time ports. */
export function canvasImageFixture() {
  const clock = new WorkflowTestClock();
  const port = new CanvasImagePortFixture();
  const budget = new CanvasImageBudget();
  const decodes = new CanvasImageDecodeScheduler(port, clock);
  const errors: unknown[] = [];
  let changes = 0;
  const resources = new CanvasImageResources({
    clock,
    budget,
    decodes,
    changed: () => {
      changes++;
    },
    logger: {
      error: (...args: unknown[]) => {
        errors.push(args);
      },
    },
  });
  return { clock, port, budget, decodes, resources, errors, changes: () => changes };
}

/** Settle promise delivery without depending on wall time or an actual image decoder. */
export async function settleCanvasImages(): Promise<void> {
  for (let index = 0; index < 8; index++) await Promise.resolve();
}
