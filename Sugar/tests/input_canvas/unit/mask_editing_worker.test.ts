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
/** Prove bounded worker scheduling, shared results, immutable submissions and explicit failures. */
import { jest } from '@jest/globals';
import {
  BrowserMaskEditing,
  type MaskEditingWorkerPort,
} from '../../../frontend/comfyui/ui/input_canvas/infrastructure/BrowserMaskEditing.js';
import { readMaskEditingRequest } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/MaskEditingTransport.js';
import { editMaskInput } from '../../../frontend/comfyui/ui/input_canvas/domain/MaskInputEditing.js';
import { DEFAULT_MASK_EDITS } from '../../../frontend/comfyui/ui/input_canvas/domain/MaskEditSettings.js';
import { encodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';

/** Replace only the external message transport while running the real mask processing policy. */
class WorkerBoundary implements MaskEditingWorkerPort {
  readonly posted: unknown[] = [];
  readonly terminate = jest.fn();
  #message: ((event: MessageEvent<unknown>) => void) | null = null;
  #error: ((event: ErrorEvent) => void) | null = null;
  /** Record only requests admitted by the processing owner. */
  postMessage(value: unknown): void {
    this.posted.push(value);
  }
  addEventListener(type: 'message', listener: (event: MessageEvent<unknown>) => void): void;
  addEventListener(type: 'error', listener: (event: ErrorEvent) => void): void;
  /** Retain the same listener shape as the native worker boundary. */
  addEventListener(
    type: 'message' | 'error',
    listener: ((event: MessageEvent<unknown>) => void) | ((event: ErrorEvent) => void),
  ): void {
    // The native overload couples each event name to its corresponding callback type.
    if (type === 'message') this.#message = listener as (event: MessageEvent<unknown>) => void;
    else this.#error = listener as (event: ErrorEvent) => void;
  }
  /** Finish the admitted request using the real domain operation. */
  finish(): void {
    const request = readMaskEditingRequest(this.posted.shift());
    this.reply({ id: request.id, result: editMaskInput(request.source, request.settings) });
  }
  /** Exercise untyped worker response validation. */
  reply(value: unknown): void {
    this.#message?.(new MessageEvent('message', { data: value }));
  }
  /** Report a terminal worker failure as a native error event. */
  fail(): void {
    this.#error?.(new ErrorEvent('error', { message: 'worker failed' }));
  }
}

/** Build a source with a soft edge and transparent support, keeping decoded bytes deterministic. */
function source() {
  return encodeCanvasRaster(3, 2, 'coverage8', Uint8Array.from([0, 64, 255, 0, 0, 128]));
}

test('shares pending and completed work without creating a worker until editing is requested', async () => {
  const worker = new WorkerBoundary(),
    create = jest.fn(() => worker),
    engine = new BrowserMaskEditing(create);
  expect(create).not.toHaveBeenCalled();
  const mask = source(),
    first = engine.prepare(mask, DEFAULT_MASK_EDITS, 'preview');
  expect(engine.prepare(mask, DEFAULT_MASK_EDITS, 'submission')).toBe(first);
  expect(engine.prepare({ ...mask }, DEFAULT_MASK_EDITS, 'submission')).toBe(first);
  expect(worker.posted).toHaveLength(1);
  worker.finish();
  expect(await first).toEqual(editMaskInput(mask, DEFAULT_MASK_EDITS));
  expect(await engine.prepare(mask, DEFAULT_MASK_EDITS, 'preview')).toBe(await first);
  expect(await engine.prepare({ ...mask }, DEFAULT_MASK_EDITS, 'submission')).toBe(await first);
  expect(worker.posted).toHaveLength(0);
  expect(create).toHaveBeenCalledTimes(1);
  engine.dispose();
  expect(worker.terminate).toHaveBeenCalledTimes(1);
});

test('prioritizes submitted inputs while retaining independent queued previews of the same source', async () => {
  const worker = new WorkerBoundary(),
    engine = new BrowserMaskEditing(() => worker),
    mask = source();
  const first = engine.prepare(mask, { ...DEFAULT_MASK_EDITS, feather: 1 }, 'preview');
  const preview = engine.prepare(mask, { ...DEFAULT_MASK_EDITS, feather: 2 }, 'preview');
  const submitted = engine.prepare(mask, { ...DEFAULT_MASK_EDITS, crop_factor: 1 }, 'submission');
  expect(worker.posted).toHaveLength(1);
  worker.finish();
  await first;
  expect(readMaskEditingRequest(worker.posted[0]).settings.crop_factor).toBe(1);
  worker.finish();
  await submitted;
  expect(readMaskEditingRequest(worker.posted[0]).settings.feather).toBe(2);
  worker.finish();
  await preview;
  engine.dispose();
});

test('rejects invalid response placement and permits the next independent request to proceed', async () => {
  const worker = new WorkerBoundary(),
    engine = new BrowserMaskEditing(() => worker),
    mask = source();
  const failed = engine.prepare(mask, DEFAULT_MASK_EDITS, 'preview');
  const assertion = expect(failed).rejects.toThrow('placement');
  const next = engine.prepare(mask, { ...DEFAULT_MASK_EDITS, feather: 1 }, 'submission');
  const request = readMaskEditingRequest(worker.posted.shift());
  const edited = editMaskInput(mask, DEFAULT_MASK_EDITS);
  worker.reply({ id: request.id, result: { ...edited, bounds: { ...edited.bounds, left: 100 } } });
  await assertion;
  worker.finish();
  expect((await next).mask.format).toBe('coverage8');
  engine.dispose();
});

test('worker failures reject active and queued callers instead of leaving previews or submissions pending', async () => {
  const worker = new WorkerBoundary(),
    engine = new BrowserMaskEditing(() => worker),
    mask = source();
  const first = engine.prepare(mask, DEFAULT_MASK_EDITS, 'preview');
  const second = engine.prepare(mask, { ...DEFAULT_MASK_EDITS, feather: 1 }, 'submission');
  const checks = [
    expect(first).rejects.toThrow('worker failed'),
    expect(second).rejects.toThrow('worker failed'),
  ];
  worker.fail();
  await Promise.all(checks);
  expect(worker.terminate).toHaveBeenCalledTimes(1);
  engine.dispose();
  await expect(engine.prepare(mask, DEFAULT_MASK_EDITS, 'preview')).rejects.toThrow('closed');
});
