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
/** Observe authoritative server jobs and tie cancellation to their requesting presentation. */
import type { ModelTransferObserver } from './ModelTransfer.js';
import type { ModelApiFetch } from './ModelApiTransport.js';
import { isRecord } from '../types/common.js';

interface ModelJobState {
  readonly id: string;
  readonly status: 'queued' | 'running' | 'completed' | 'failed' | 'cancelled';
  readonly error: string;
  readonly result: Record<string, unknown>;
  readonly percentage: number | null;
}

/** Share job validation, polling, and cancellation across metadata and acquisition actions. */
export class ModelJobClient {
  readonly #fetch: ModelApiFetch;

  /** Bind the same host transport as the catalog without depending on node presentation. */
  constructor(fetchApi: ModelApiFetch) {
    this.#fetch = fetchApi;
  }

  /** Return completed result evidence and cancel an active server job when its view closes. */
  async observe(
    response: Response,
    messages: { queued: string; running: string },
    progress: (message: string) => void,
    signal: AbortSignal,
    transfer?: ModelTransferObserver,
  ): Promise<Record<string, unknown>> {
    let job = decodeJob(await response.json());
    const cancel = (): void => {
      void this.#fetch(`/sugarcubes/v1/models/jobs/${encodeURIComponent(job.id)}`, {
        method: 'DELETE',
      }).catch((error: unknown) =>
        console.error('SugarCubes model job cancellation failed.', error),
      );
    };
    if (signal.aborted) {
      cancel();
      throw new DOMException('Cancelled', 'AbortError');
    }
    signal.addEventListener('abort', cancel, { once: true });
    try {
      while (job.status === 'queued' || job.status === 'running') {
        transfer?.({
          message: job.status === 'queued' ? messages.queued : messages.running,
          percentage: job.percentage,
        });
        progress(
          job.status === 'queued'
            ? messages.queued
            : messages.running + (job.percentage === null ? '' : ` ${job.percentage}%`),
        );
        await abortableModelDelay(400, signal);
        const next = await this.#fetch(`/sugarcubes/v1/models/jobs/${encodeURIComponent(job.id)}`, {
          signal,
        });
        if (!next.ok) throw new Error('Model job status is unavailable.');
        job = decodeJob(await next.json());
      }
      if (job.status !== 'completed')
        throw new Error(job.error || 'The model operation did not complete.');
      return job.result;
    } finally {
      signal.removeEventListener('abort', cancel);
    }
  }
}

/** Reject unknown job identities and states before addressing server resources. */
function decodeJob(value: unknown): ModelJobState {
  if (!isRecord(value) || typeof value.jobId !== 'string' || !/^[a-f0-9]{32}$/.test(value.jobId))
    throw new Error('Invalid model job response.');
  const status = value.status;
  if (
    status !== 'queued' &&
    status !== 'running' &&
    status !== 'completed' &&
    status !== 'failed' &&
    status !== 'cancelled'
  )
    throw new Error('Invalid model job state.');
  return {
    id: value.jobId,
    status,
    error: typeof value.error === 'string' ? value.error : '',
    result: isRecord(value.result) ? value.result : {},
    percentage: decodeProgress(value.progress),
  };
}

/** Validate streamed byte evidence without trusting caller-supplied progress strings. */
function decodeProgress(value: unknown): number | null {
  if (value === undefined || (isRecord(value) && Object.keys(value).length === 0)) return null;
  if (!isRecord(value)) throw new Error('Invalid model transfer progress.');
  const downloaded = value.bytesDownloaded;
  const total = value.bytesTotal;
  if (
    typeof downloaded !== 'number' ||
    typeof total !== 'number' ||
    !Number.isSafeInteger(downloaded) ||
    !Number.isSafeInteger(total) ||
    downloaded < 0 ||
    total <= 0 ||
    downloaded > 128 * 1024 ** 3 + 1024 ||
    total > 128 * 1024 ** 3
  )
    throw new Error('Invalid model transfer progress.');
  return Math.min(100, Math.floor((downloaded / total) * 100));
}

/** Release retry and polling timers immediately when the owning presentation closes. */
export function abortableModelDelay(milliseconds: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) {
      reject(new DOMException('Cancelled', 'AbortError'));
      return;
    }
    const abort = (): void => {
      clearTimeout(timer);
      reject(new DOMException('Cancelled', 'AbortError'));
    };
    const timer = setTimeout(() => {
      signal.removeEventListener('abort', abort);
      resolve();
    }, milliseconds);
    signal.addEventListener('abort', abort, { once: true });
  });
}
