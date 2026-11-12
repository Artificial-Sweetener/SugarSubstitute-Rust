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
/** Read exact retained originals through the current host's bounded same-origin capability. */
import type {
  SessionBinding,
  SessionOutput,
} from '../../../../integrations/contracts/SessionProtocol.generated.js';
import { isSessionBinding } from '../../../../integrations/shared/SessionValidation.generated.js';
import { isSessionOutput } from '../../../../integrations/shared/SessionOutputValidation.generated.js';
import { sessionOutput } from '../../../../integrations/shared/SessionSnapshot.js';
import { MAX_CANVAS_IMAGE_BYTES } from '../../../../integrations/shared/CanvasImagePolicy.js';
import type { OutputFileReference, ImageDimensions } from '../execution/domain/WorkflowOutput.js';
import { outputImageDimensions, outputImageTransferDimensions } from './OutputImageDimensions.js';

/** Carry exact immutable artifact facts beside a fresh revocable native URL, never a filesystem path. */
export interface BrowserRetainedOutputSource {
  readonly output: SessionOutput;
  readonly binding: SessionBinding;
  readonly url: string;
}

/** Advertise only actions the retained reader can actually complete within existing byte budgets. */
export function retainedOutputCapabilities(source: BrowserRetainedOutputSource): {
  readonly copy: boolean;
  readonly drag: boolean;
  readonly download: boolean;
  readonly reveal: false;
  readonly photoshop: false;
} {
  const bounded = source.output.bytes > 0 && source.output.bytes <= MAX_CANVAS_IMAGE_BYTES;
  const image =
    source.output.media === 'image' &&
    ['image/png', 'image/jpeg', 'image/webp'].includes(source.output.mime) &&
    outputImageTransferDimensions(source.output);
  return Object.freeze({
    copy: bounded && image,
    drag: bounded && image,
    download: bounded,
    reveal: false,
    photoshop: false,
  });
}

/** Keep confirmed source bytes separate from a requested clipboard rendition. */
export interface RetainedOutputImage {
  readonly blob: Blob;
  readonly filename: string;
  readonly mimeType: 'image/png' | 'image/jpeg' | 'image/webp';
  readonly dimensions: ImageDimensions;
  readonly url: string;
}

/** Fetch only native-issued retained capabilities and authenticate complete bounded image bodies. */
export class BrowserRetainedOutputTransfer {
  readonly #pending = new Set<AbortController>();
  #disposed = false;
  /** Bind the current frontend origin, HTTP boundary and interoperable SHA-256 implementation. */
  constructor(
    private readonly origin: string,
    private readonly fetch: typeof globalThis.fetch,
    private readonly digest: (bytes: Uint8Array<ArrayBuffer>) => Promise<string> = sha256,
  ) {}

  /** Read one exact original for supported image clipboard and drag operations. */
  async image(
    file: OutputFileReference,
    source: BrowserRetainedOutputSource,
    signal?: AbortSignal,
  ): Promise<RetainedOutputImage> {
    if (!retainedOutputCapabilities(source).copy)
      throw new Error('This retained output exceeds the supported image transfer bounds.');
    const url = this.#url(file, source);
    const mime = source.output.mime;
    if (mime !== 'image/png' && mime !== 'image/jpeg' && mime !== 'image/webp')
      throw new Error('This retained output is not a supported clipboard image.');
    return this.#request(signal, async (controller) => {
      const response = await this.fetch(url, { signal: controller.signal });
      this.#headers(response, source);
      if (!response.body) throw new Error('The retained output response is empty.');
      const reader = response.body.getReader();
      const data = new Uint8Array(source.output.bytes);
      let offset = 0;
      try {
        for (;;) {
          if (controller.signal.aborted) throw new DOMException('Cancelled', 'AbortError');
          const next = await reader.read();
          if (next.done) break;
          if (offset + next.value.byteLength > data.byteLength)
            throw new Error('The retained output exceeded its confirmed length.');
          data.set(next.value, offset);
          offset += next.value.byteLength;
        }
      } finally {
        try {
          await reader.cancel();
        } finally {
          reader.releaseLock();
        }
      }
      if (offset !== data.byteLength || (await this.digest(data)) !== source.output.digest)
        throw new Error('The retained output differs from its confirmed identity.');
      if (controller.signal.aborted || this.#disposed)
        throw new DOMException('Cancelled', 'AbortError');
      const dimensions = outputImageDimensions(data, mime);
      if (dimensions.width !== source.output.width || dimensions.height !== source.output.height)
        throw new Error('The retained output geometry differs from its confirmed identity.');
      return {
        blob: new Blob([data], { type: mime }),
        filename: source.output.file.filename,
        mimeType: mime,
        dimensions,
        url,
      };
    });
  }

  /** Validate exact native metadata before delegating a bounded original download to the browser. */
  async download(
    file: OutputFileReference,
    source: BrowserRetainedOutputSource,
    signal?: AbortSignal,
  ): Promise<string> {
    if (!retainedOutputCapabilities(source).download)
      throw new Error('This retained output exceeds the supported direct download limit.');
    const url = this.#url(file, source);
    return this.#request(signal, async (controller) => {
      const response = await this.fetch(url, { method: 'HEAD', signal: controller.signal });
      this.#headers(response, source);
      return url;
    });
  }

  /** Revoke pending reads and refuse all later publication after the owning canvas is retired. */
  dispose(): void {
    this.#disposed = true;
    for (const controller of this.#pending) controller.abort();
  }

  /** Validate native URL correlation; the host separately checks the actual capability and current custody. */
  #url(file: OutputFileReference, source: BrowserRetainedOutputSource): string {
    if (
      !isSessionOutput(source.output) ||
      !sessionOutput(source.output) ||
      !isSessionBinding(source.binding) ||
      source.output.assetDigest !== source.output.digest ||
      source.binding.entryId !== source.output.entryId ||
      file.filename !== source.output.file.filename ||
      file.subfolder !== source.output.file.subfolder ||
      file.type !== source.output.file.storage
    )
      throw new Error('The retained output source is invalid.');
    const url = new URL(source.url, this.origin);
    const expected = new URL(this.origin);
    const path = /^\/__sugarsubstitute\/session-media\/v1\/[0-9a-f]{32}\/([0-9a-f-]{36})$/.exec(
      url.pathname,
    );
    if (
      url.origin !== expected.origin ||
      !['http:', 'https:'].includes(url.protocol) ||
      url.username ||
      url.password ||
      url.hash ||
      url.href.length > 4096 ||
      path?.[1] !== source.output.id ||
      [...url.searchParams.keys()].length !== 1 ||
      !url.searchParams.has('binding')
    )
      throw new Error('The retained output URL is outside its current host scope.');
    let binding: unknown;
    try {
      binding = JSON.parse(url.searchParams.get('binding') ?? '');
    } catch {
      throw new Error('The retained output binding is invalid.');
    }
    if (
      !isSessionBinding(binding) ||
      !(
        [
          'entryId',
          'runtimeSession',
          'generation',
          'graphRevision',
          'sourceRevision',
          'navigationRevision',
        ] as const
      ).every((key) => binding[key] === source.binding[key])
    )
      throw new Error('The retained output binding differs from its current source.');
    return url.href;
  }

  /** Refuse partial or mismatched original metadata before any byte or clipboard publication. */
  #headers(response: Response, source: BrowserRetainedOutputSource): void {
    if (
      !response.ok ||
      response.status !== 200 ||
      response.headers.get('Content-Type') !== source.output.mime ||
      response.headers.get('Content-Length') !== String(source.output.bytes) ||
      response.headers.has('Content-Range')
    )
      throw new Error('The retained output response does not match its confirmed original.');
  }

  /** Admit two active bounded reads; cancellation retains each operation through its observed terminal outcome. */
  async #request<T>(
    signal: AbortSignal | undefined,
    operation: (controller: AbortController) => Promise<T>,
  ): Promise<T> {
    if (this.#disposed || signal?.aborted) throw new DOMException('Cancelled', 'AbortError');
    if (this.#pending.size >= 2) throw new Error('Retained output transfers are busy.');
    const controller = new AbortController();
    const abort = () => controller.abort();
    signal?.addEventListener('abort', abort, { once: true });
    this.#pending.add(controller);
    const timeout = setTimeout(abort, 20000);
    try {
      return await operation(controller);
    } finally {
      clearTimeout(timeout);
      signal?.removeEventListener('abort', abort);
      this.#pending.delete(controller);
    }
  }
}

/** Hash complete bounded original bytes using the browser's cryptographic owner. */
async function sha256(bytes: Uint8Array<ArrayBuffer>): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return [...new Uint8Array(digest)].map((value) => value.toString(16).padStart(2, '0')).join('');
}
