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
/** Admit one contiguous bounded checkpoint transfer independently of browser and native transport. */
import {
  decodeDesktopCheckpoint,
  utf8Bytes,
  SESSION_CHUNK_BYTES,
} from '../../core/embedded/SessionRuntime.js';
import type {
  SessionCheckpoint,
  SessionTransfer,
} from '../../../../../integrations/contracts/SessionProtocol.generated.js';

interface IncomingTransfer {
  readonly requestId: string;
  readonly descriptor: SessionTransfer;
  readonly chunks: string[];
  bytes: number;
  committed: boolean;
}

/** Retain at most one admitted encoded checkpoint, releasing it on cancellation and consumption. */
export class DesktopSessionTransfer {
  #incoming: IncomingTransfer | null = null;

  /** Inject byte hashing rather than choosing browser, Node or native cryptography here. */
  constructor(private readonly digest: (text: string) => Promise<string>) {}

  /** Refuse overlapping transfer admission instead of silently replacing retained original bytes. */
  begin(requestId: string, descriptor: SessionTransfer): void {
    if (this.#incoming) throw new Error('A desktop checkpoint transfer is already active.');
    if (
      descriptor.direction !== 'materialize' ||
      descriptor.bytes > 134217728 ||
      descriptor.bytes < 1 ||
      descriptor.chunks < 1 ||
      descriptor.chunks > 8192 ||
      descriptor.bytes < descriptor.chunks ||
      descriptor.bytes > descriptor.chunks * SESSION_CHUNK_BYTES
    )
      throw new TypeError('Desktop checkpoint transfer admission is invalid.');
    this.#incoming = { requestId, descriptor, chunks: [], bytes: 0, committed: false };
  }

  /** Require exact contiguous chunk order and charged UTF-8 bytes before retaining the text. */
  chunk(requestId: string, transferId: string, index: number, text: string): void {
    const incoming = this.#require(requestId, transferId);
    const bytes = utf8Bytes(text);
    if (
      incoming.committed ||
      index !== incoming.chunks.length ||
      index >= incoming.descriptor.chunks ||
      bytes < 1 ||
      bytes > SESSION_CHUNK_BYTES ||
      incoming.bytes + bytes > incoming.descriptor.bytes
    )
      throw new TypeError('Desktop checkpoint chunk is out of order or exceeds admission.');
    incoming.chunks.push(text);
    incoming.bytes += bytes;
  }

  /** Mark only an exactly complete encoded transfer ready for later fenced materialization. */
  commit(requestId: string, transferId: string): void {
    const incoming = this.#require(requestId, transferId);
    if (
      incoming.committed ||
      incoming.bytes !== incoming.descriptor.bytes ||
      incoming.chunks.length !== incoming.descriptor.chunks
    )
      throw new TypeError('Desktop checkpoint transfer is incomplete.');
    incoming.committed = true;
  }

  /** Hash and decode after byte admission; a cancelled transfer can never revive on delayed digest completion. */
  async take(requestId: string, transferId: string): Promise<SessionCheckpoint> {
    const incoming = this.#require(requestId, transferId);
    if (!incoming.committed) throw new Error('Desktop checkpoint transfer is not committed.');
    const text = incoming.chunks.join('');
    const digest = await this.digest(text);
    if (this.#incoming !== incoming) throw new Error('Desktop checkpoint transfer was cancelled.');
    const checkpoint = decodeDesktopCheckpoint(text);
    if (
      digest !== incoming.descriptor.digest ||
      !checkpoint ||
      checkpoint.id !== incoming.descriptor.checkpointId ||
      checkpoint.revision !== incoming.descriptor.revision
    )
      throw new TypeError('Desktop checkpoint identity or content validation failed.');
    this.#incoming = null;
    return checkpoint;
  }

  /** Release only the correlated request, leaving a different admitted transfer untouched. */
  cancel(requestId: string): void {
    if (this.#incoming?.requestId === requestId) this.#incoming = null;
  }

  /** Release all encoded bytes at page teardown. */
  dispose(): void {
    this.#incoming = null;
  }

  /** Require both operation and transfer identity on every incoming effect. */
  #require(requestId: string, transferId: string): IncomingTransfer {
    const incoming = this.#incoming;
    if (incoming?.requestId !== requestId || incoming.descriptor.id !== transferId)
      throw new Error('Desktop checkpoint transfer is stale.');
    return incoming;
  }
}

/** Split by Unicode scalar so each wire chunk stays within raw UTF-8 admission. */
export function desktopSessionChunks(text: string): readonly string[] {
  const chunks: string[] = [];
  let current = '';
  let bytes = 0;
  for (const character of text) {
    const count = utf8Bytes(character);
    if (!Number.isFinite(count))
      throw new TypeError('Desktop checkpoint contains invalid Unicode.');
    if (bytes + count > SESSION_CHUNK_BYTES) {
      chunks.push(current);
      current = '';
      bytes = 0;
    }
    current += character;
    bytes += count;
  }
  if (current) chunks.push(current);
  return Object.freeze(chunks);
}
