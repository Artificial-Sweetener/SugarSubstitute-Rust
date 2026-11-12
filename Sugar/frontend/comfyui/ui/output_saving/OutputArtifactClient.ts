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
/** Confirm exact output artifacts through the backend filesystem authority. */
import type {
  SessionFile,
  SessionOutput,
} from '../../../../integrations/contracts/SessionProtocol.generated.js';
import { isSessionFile } from '../../../../integrations/shared/SessionOutputValidation.generated.js';
import { sessionDigest, sessionFile } from '../../../../integrations/shared/SessionPrimitives.js';
import type { OutputFileReference } from '../execution/domain/WorkflowOutput.js';
import { isRecord } from '../types/common.js';

/** Confirmed encoded identity; host asset publication remains a separate durable operation. */
export interface OutputArtifactReceipt {
  readonly file: SessionFile;
  readonly digest: string;
  readonly bytes: number;
  readonly width: number;
  readonly height: number;
  readonly media: 'image' | 'video';
  readonly mime: string;
  readonly metadataEmbedded: boolean;
  readonly animated: boolean;
}

/** Stable per-file resolver outcomes that never erase checkpoint membership. */
export type OutputArtifactUnavailableCode =
  | 'missing'
  | 'changed'
  | 'unsupported'
  | 'inaccessible'
  | 'invalid_reference'
  | 'budget'
  | 'cancelled'
  | 'ambiguous';

/** Distinguish an available confirmed descriptor from an actionable per-item refusal. */
export type OutputArtifactResult =
  | { readonly available: true; readonly receipt: OutputArtifactReceipt }
  | {
      readonly available: false;
      readonly code: OutputArtifactUnavailableCode;
      readonly message: string;
    };

const unavailableCodes: ReadonlySet<string> = new Set([
  'missing',
  'changed',
  'unsupported',
  'inaccessible',
  'invalid_reference',
  'budget',
  'cancelled',
  'ambiguous',
]);

/** Isolate bounded HTTP and validation from output membership, source provenance and checkpoints. */
export class OutputArtifactClient {
  /** Bind an explicit current-origin transport; descriptor values never select a backend. */
  constructor(private readonly fetch: typeof globalThis.fetch) {}

  /** Confirm a live final's encoded identity before a host checkpoint can claim durability. */
  async receipt(file: OutputFileReference, signal?: AbortSignal): Promise<OutputArtifactResult> {
    const result = await this.#request('artifact-receipt', { file }, signal);
    if (
      result.available &&
      (result.receipt.file.filename !== file.filename ||
        result.receipt.file.subfolder !== file.subfolder.replaceAll('\\', '/') ||
        result.receipt.file.storage !== file.type)
    )
      throw new Error('Confirmed output differs from its requested descriptor.');
    return result;
  }

  /** Re-resolve only a recorded artifact; source/run ownership remains the session domain's concern. */
  async resolve(output: SessionOutput, signal?: AbortSignal): Promise<OutputArtifactResult> {
    const { file, digest, bytes, width, height, media, mime, metadataEmbedded, promptId } = output;
    const result = await this.#request(
      'artifact-resolve',
      {
        expected: { file, digest, bytes, width, height, media, mime, metadataEmbedded, promptId },
      },
      signal,
    );
    if (result.available) {
      const actual = result.receipt;
      if (
        actual.digest !== digest ||
        actual.bytes !== bytes ||
        actual.width !== width ||
        actual.height !== height ||
        actual.media !== media ||
        actual.mime !== mime ||
        actual.metadataEmbedded !== metadataEmbedded
      )
        throw new Error('Resolved output differs from its recorded identity.');
    }
    return result;
  }

  /** Bound response allocation and lifetime while keeping per-file refusal an ordinary typed result. */
  async #request(
    action: string,
    body: unknown,
    signal?: AbortSignal,
  ): Promise<OutputArtifactResult> {
    const controller = new AbortController();
    const abort = () => controller.abort();
    if (signal?.aborted) abort();
    signal?.addEventListener('abort', abort, { once: true });
    const timer = setTimeout(abort, 20000);
    try {
      const response = await this.fetch('/sugarcubes/output-saving/' + action, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
        signal: controller.signal,
      });
      const encoded = await boundedResponse(response, 16384);
      const value: unknown = JSON.parse(encoded);
      const result = decodeArtifactResult(value);
      if (!response.ok && result.available) throw new Error('Invalid output artifact response.');
      return result;
    } finally {
      clearTimeout(timer);
      signal?.removeEventListener('abort', abort);
    }
  }
}

/** Decode the exact response envelope; unknown fields and inconsistent managed IDs fail closed. */
export function decodeArtifactResult(value: unknown): OutputArtifactResult {
  if (!isRecord(value)) throw new Error('Invalid output artifact response.');
  if (
    value.available === false &&
    keys(value, ['available', 'code', 'message']) &&
    typeof value.code === 'string' &&
    unavailableCodes.has(value.code) &&
    typeof value.message === 'string' &&
    value.message.length <= 2048
  ) {
    return Object.freeze({
      available: false,
      code: value.code as OutputArtifactUnavailableCode,
      message: value.message,
    });
  }
  if (
    value.available !== true ||
    !keys(value, ['available', 'receipt']) ||
    !isRecord(value.receipt)
  )
    throw new Error('Invalid output artifact response.');
  const receipt = value.receipt;
  const { file, digest, bytes, width, height, media, mime, metadataEmbedded, animated } = receipt;
  if (
    !keys(receipt, [
      'file',
      'digest',
      'bytes',
      'width',
      'height',
      'media',
      'mime',
      'metadataEmbedded',
      'animated',
    ]) ||
    !isSessionFile(file) ||
    !sessionFile(file) ||
    !managedFile(file) ||
    typeof digest !== 'string' ||
    !sessionDigest(digest) ||
    !integer(bytes, 8589934592) ||
    !integer(width, 131072) ||
    !integer(height, 131072) ||
    (media !== 'image' && media !== 'video') ||
    !['image/png', 'image/jpeg', 'image/webp', 'image/gif', 'video/mp4', 'video/webm'].includes(
      String(mime),
    ) ||
    typeof metadataEmbedded !== 'boolean' ||
    typeof animated !== 'boolean' ||
    typeof mime !== 'string' ||
    mime.startsWith('video/') !== (media === 'video') ||
    (media === 'video' && animated) ||
    (mime === 'image/gif' && !animated) ||
    (mime === 'image/jpeg' && animated) ||
    !matchingMime(file.filename, mime)
  )
    throw new Error('Invalid output artifact response.');
  return Object.freeze({
    available: true,
    receipt: Object.freeze({
      file: Object.freeze({ ...file }),
      digest,
      bytes,
      width,
      height,
      media,
      mime,
      metadataEmbedded,
      animated,
    }),
  });
}

/** Require encoded identity to agree with the supported filename representation. */
function matchingMime(filename: string, mime: string): boolean {
  const suffix = filename.split('.').at(-1)?.toLowerCase();
  return (
    (mime === 'image/png' && (suffix === 'png' || suffix === 'apng')) ||
    (mime === 'image/jpeg' && (suffix === 'jpg' || suffix === 'jpeg')) ||
    (mime === 'image/webp' && suffix === 'webp') ||
    (mime === 'image/gif' && suffix === 'gif') ||
    (mime === 'video/mp4' && suffix === 'mp4') ||
    (mime === 'video/webm' && suffix === 'webm')
  );
}
/** Require one canonical registry token consistent with the established managed view namespace. */
function managedFile(file: SessionFile): boolean {
  const prefix = '__sugarcubes_managed__/';
  if (!file.subfolder.startsWith(prefix)) return file.managedArtifact === null;
  const identity = file.subfolder.slice(prefix.length);
  return (
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(identity) &&
    identity !== '00000000-0000-0000-0000-000000000000' &&
    file.managedArtifact === identity
  );
}

/** Require the closed JSON shape without accepting inherited authority-like members. */
function keys(value: Record<string, unknown>, names: readonly string[]): boolean {
  return (
    Object.keys(value).length === names.length && names.every((name) => Object.hasOwn(value, name))
  );
}

/** Admit exact safe integer bounds before runtime consumers can allocate or decode. */
function integer(value: unknown, maximum: number): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 1 && value <= maximum;
}

/** Consume small JSON responses incrementally instead of allocating an unbounded response body. */
async function boundedResponse(response: Response, maximum: number): Promise<string> {
  if (!response.body) throw new Error('Output artifact response is empty.');
  const reader = response.body.getReader();
  const decoder = new TextDecoder('utf-8', { fatal: true });
  let total = 0;
  let text = '';
  try {
    for (;;) {
      const next = await reader.read();
      if (next.done) break;
      total += next.value.byteLength;
      if (total > maximum) throw new Error('Output artifact response exceeds its byte limit.');
      text += decoder.decode(next.value, { stream: true });
    }
    return text + decoder.decode();
  } finally {
    try {
      await reader.cancel();
    } finally {
      reader.releaseLock();
    }
  }
}
