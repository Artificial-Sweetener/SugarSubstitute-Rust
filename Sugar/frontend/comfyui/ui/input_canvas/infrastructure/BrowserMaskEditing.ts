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
/** Bound asynchronous mask processing and prioritize immutable submission work. */
import type { MaskEditingPort } from '../application/MaskEditingPort.js';
import type { MaskEditSettings } from '../domain/MaskEditSettings.js';
import type { EditedMaskInput } from '../domain/MaskInputEditing.js';
import { decodeCanvasRaster, type CanvasPixelRaster } from '../../core/embedded/CanvasRuntime.js';
import { InputRasterTransformCache } from '../application/InputRasterTransformCache.js';
import { readMaskEditSettings } from '../domain/MaskEditSettings.js';
import { readMaskEditingResult } from './MaskEditingTransport.js';
import { isRecord } from '../../types/common.js';

/** Keep native worker construction replaceable at the external boundary in deterministic tests. */
export interface MaskEditingWorkerPort {
  postMessage(value: unknown): void;
  addEventListener(type: 'message', listener: (event: MessageEvent<unknown>) => void): void;
  addEventListener(type: 'error', listener: (event: ErrorEvent) => void): void;
  terminate(): void;
}

interface Job {
  readonly id: number;
  readonly source: CanvasPixelRaster;
  readonly settings: MaskEditSettings;
  readonly purpose: 'preview' | 'submission';
  readonly promise: Promise<EditedMaskInput>;
  resolve(value: EditedMaskInput): void;
  reject(error: unknown): void;
}

/** Own one worker and one byte-bounded cache for both preview and submission callers. */
export class BrowserMaskEditing implements MaskEditingPort {
  readonly #cache = new InputRasterTransformCache<EditedMaskInput>();
  readonly #pending = new Map<number, Job>();
  readonly #queue: Job[] = [];
  #nextJob = 0;
  #active: Job | null = null;
  #worker: MaskEditingWorkerPort | null = null;
  #disposed = false;
  /** Create a dedicated worker lazily so canvases without mask editing allocate none. */
  constructor(
    private readonly createWorker: () => MaskEditingWorkerPort = () =>
      new Worker(new URL('./MaskEditingWorker.js', import.meta.url), { type: 'module' }),
  ) {}

  /** Share identical work while each preview owner coalesces its own obsolete intentions. */
  prepare(
    source: CanvasPixelRaster,
    settings: MaskEditSettings,
    purpose: 'preview' | 'submission',
  ): Promise<EditedMaskInput> {
    if (this.#disposed) return Promise.reject(new Error('Mask preparation is closed.'));
    const accepted = readMaskEditSettings(settings);
    if (source.format !== 'coverage8')
      return Promise.reject(new Error('Mask preparation requires coverage.'));
    return this.#cache.read(
      [source],
      JSON.stringify(accepted),
      () => this.#schedule(source, accepted, purpose),
      (value) => value.mask.data.length * 2,
    );
  }

  /** Admit one deduplicated operation to the worker scheduling boundary. */
  #schedule(
    source: CanvasPixelRaster,
    settings: MaskEditSettings,
    purpose: 'preview' | 'submission',
  ): Promise<EditedMaskInput> {
    if (this.#pending.size >= 16)
      return Promise.reject(new Error('Too many pending mask preparations.'));
    let resolve: Job['resolve'] = () => {},
      reject: Job['reject'] = () => {};
    const promise = new Promise<EditedMaskInput>((success, failure) => {
      resolve = success;
      reject = failure;
    });
    const job: Job = {
      id: ++this.#nextJob,
      source,
      settings,
      purpose,
      promise,
      resolve,
      reject,
    };
    this.#pending.set(job.id, job);
    if (purpose === 'submission') this.#queue.unshift(job);
    else this.#queue.push(job);
    this.#drain();
    return promise;
  }

  /** Reject outstanding work before releasing the browser-owned worker. */
  dispose(): void {
    this.#disposed = true;
    this.#fail(new Error('Mask preparation is closed.'));
    this.#cache.clear();
  }

  /** Transfer only one full-resolution source at a time, keeping pending slider intents lightweight. */
  #drain(): void {
    if (this.#disposed || this.#active) return;
    const job = this.#queue.shift();
    if (!job) return;
    this.#active = job;
    try {
      if (!this.#worker) {
        this.#worker = this.createWorker();
        this.#worker.addEventListener('message', (event) => this.#receive(event.data));
        this.#worker.addEventListener('error', (event) =>
          this.#fail(new Error(event.message || 'Mask worker failed.')),
        );
      }
      decodeCanvasRaster(job.source);
      this.#worker.postMessage({ id: job.id, source: job.source, settings: job.settings });
    } catch (error: unknown) {
      this.#fail(error);
    }
  }

  /** Admit the exact current result and evict retained pixels before exceeding the cache budget. */
  #receive(value: unknown): void {
    const job = this.#active;
    if (!job) return;
    try {
      if (!isRecord(value) || value.id !== job.id)
        throw new Error('Invalid mask worker response identity.');
      if (typeof value.error === 'string') throw new Error(value.error);
      const result = readMaskEditingResult(value.result, job.source);
      job.resolve(result);
    } catch (error: unknown) {
      job.reject(error);
    }
    this.#pending.delete(job.id);
    this.#active = null;
    this.#drain();
  }

  /** Fail explicit callers together when the external worker cannot continue. */
  #fail(error: unknown): void {
    this.#worker?.terminate();
    this.#worker = null;
    for (const job of this.#pending.values()) job.reject(error);
    this.#pending.clear();
    this.#queue.length = 0;
    this.#active = null;
  }
}
