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
/** Resolve portable output representations through the backend's shared transfer authority. */
import type { OutputFileReference } from '../execution/domain/WorkflowOutput.js';
import { isRecord } from '../types/common.js';

export interface OutputTransferSelection {
  readonly file: OutputFileReference;
  readonly mimeType: 'image/png' | 'image/jpeg' | 'image/webp';
}

/** Keep network access and response validation independent of a canvas or operating system. */
export class OutputTransferClient {
  /** Bind an explicit same-origin transport rather than reading renderer-owned preferences. */
  constructor(private readonly fetch: typeof globalThis.fetch) {}

  /** Revalidate a file and apply current preferences with bounded, caller-cancellable network work. */
  async choose(file: OutputFileReference, signal?: AbortSignal): Promise<OutputTransferSelection> {
    const controller = new AbortController();
    const abort = () => controller.abort();
    signal?.addEventListener('abort', abort, { once: true });
    if (signal?.aborted) abort();
    const timer = setTimeout(abort, 20000);
    try {
      const response = await this.fetch('/sugarcubes/output-saving/transfer', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(file),
        signal: controller.signal,
      });
      const value: unknown = await response.json();
      if (!response.ok || !isRecord(value) || value.success !== true) {
        const message = isRecord(value) && isRecord(value.error) ? value.error.message : null;
        throw new Error(
          typeof message === 'string' ? message : 'This output image is unavailable for transfer.',
        );
      }
      return decodeSelection(value.data);
    } finally {
      clearTimeout(timer);
      signal?.removeEventListener('abort', abort);
    }
  }
}

/** Preserve full-fidelity file URLs without native lossy preview parameters. */
export function outputFileUrl(origin: string, file: OutputFileReference): string {
  const url = new URL('/view', origin);
  if (url.protocol !== 'http:' && url.protocol !== 'https:')
    throw new Error('The image host is unavailable.');
  url.search = new URLSearchParams({ ...file }).toString();
  return url.href;
}

/** Admit exact portable image identities without accepting external URLs or filesystem paths. */
function decodeSelection(value: unknown): OutputTransferSelection {
  if (!isRecord(value) || !isRecord(value.file))
    throw new Error('Invalid output transfer response.');
  const { filename, subfolder, type } = value.file;
  if (
    typeof filename !== 'string' ||
    !filename ||
    /[/\\:]/.test(filename) ||
    filename.includes('\0') ||
    typeof subfolder !== 'string' ||
    subfolder.includes(':') ||
    subfolder.includes('\0') ||
    subfolder.startsWith('/') ||
    subfolder.split(/[/\\]/).some((part) => part === '.' || part === '..') ||
    (type !== 'input' && type !== 'output' && type !== 'temp') ||
    (value.mime_type !== 'image/png' &&
      value.mime_type !== 'image/jpeg' &&
      value.mime_type !== 'image/webp')
  )
    throw new Error('Invalid output transfer response.');
  return Object.freeze({
    file: Object.freeze({ filename, subfolder, type }),
    mimeType: value.mime_type,
  });
}
