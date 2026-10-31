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
/** Bound preview byte mechanics without owning a workflow, transfer cursor or native camera. */
import type {
  CanvasSurfaceCapabilities,
  CanvasSurfaceCommand,
  CanvasSurfacePresentationChange,
} from '../contracts/CanvasSurface.js';
import type { CanvasMessage } from '../contracts/CanvasProtocol.generated.js';
import { decodeCanvasBytes } from './CanvasByteEncoding.js';
import { MAX_CANVAS_IMAGE_BYTES } from './CanvasImagePolicy.js';
import { snapshotCanvasSurfaceViewport } from './CanvasSurfaceViewport.js';

/** Fit bounded base64 chunks and maximum attachment identities inside the existing native envelope. */
export const CANVAS_PREVIEW_CHUNK_BYTES = 8192;
/** Bound staging and preparation by the initial native monotonic deadline, without progress renewal. */
export const CANVAS_PREVIEW_LEASE_MILLISECONDS = 30000;
/** Preserve the current native public JSON bound; its adapter also bounds escaped transport framing. */
export const CANVAS_PREVIEW_MESSAGE_BYTES = 16384;

/** Identify mutations whose successful replies acknowledge custody rather than native presentation. */
export function isCanvasPreviewCommand(command: CanvasSurfaceCommand): boolean {
  return command.type.startsWith('surface.preview.');
}

/** Admit a declared encoding against the host while its lease owns subsequent byte interpretation. */
export function supportsCanvasSurfacePreview(
  capabilities: CanvasSurfaceCapabilities,
  command: CanvasSurfaceCommand,
): boolean {
  if (!isCanvasPreviewCommand(command)) return true;
  return command.type === 'surface.preview.begin'
    ? capabilities.previewFormats.includes(command.format)
    : capabilities.previewFormats.length > 0;
}

/** Validate immutable attribution and exact chunk encoding before a host receives preview data. */
export function validCanvasPreviewCommand(command: CanvasSurfaceCommand): boolean {
  if (command.type === 'surface.preview.begin') {
    const preview = command.preview;
    return (
      command.byteLength > 0 &&
      command.byteLength <= MAX_CANVAS_IMAGE_BYTES &&
      preview.token === `preview:${String(preview.sequence)}` &&
      preview.promptId.startsWith('prompt:') &&
      preview.promptId.length > 7 &&
      preview.sourceId.startsWith('source:') &&
      preview.sourceId.length > 7 &&
      !/[\u0000-\u001f\u007f-\u009f]/.test(preview.promptId + preview.sourceId)
    );
  }
  if (command.type === 'surface.preview.chunk') {
    try {
      return decodeCanvasBytes(command.data, CANVAS_PREVIEW_CHUNK_BYTES).length > 0;
    } catch {
      return false;
    }
  }
  return true;
}

/** Decode only one exact canonical chunk; the host lease remains the sole expected-offset owner. */
export function decodeCanvasPreviewChunk(data: string, offset: number, total: number): Uint8Array {
  if (
    !Number.isSafeInteger(total) ||
    total < 1 ||
    total > MAX_CANVAS_IMAGE_BYTES ||
    !Number.isSafeInteger(offset) ||
    offset < 0 ||
    offset >= total ||
    offset % CANVAS_PREVIEW_CHUNK_BYTES !== 0
  )
    throw new RangeError('Canvas preview chunk exceeds its lease bounds.');
  const bytes = decodeCanvasBytes(data, CANVAS_PREVIEW_CHUNK_BYTES);
  if (bytes.length !== Math.min(CANVAS_PREVIEW_CHUNK_BYTES, total - offset))
    throw new RangeError('Canvas preview chunk does not match its declared byte range.');
  return bytes;
}

/** Check serialized public JSON without importing an adapter's private transport framing. */
export function canvasPreviewMessageFits(message: CanvasMessage): boolean {
  const encoded = JSON.stringify(message);
  let bytes = 0;
  for (const character of encoded) {
    const point = character.codePointAt(0) ?? 0;
    bytes += point <= 0x7f ? 1 : point <= 0x7ff ? 2 : point <= 0xffff ? 3 : 4;
    if (bytes > CANVAS_PREVIEW_MESSAGE_BYTES) return false;
  }
  return true;
}

/** Preserve a native observation without retaining mutable adapter-owned metadata. */
export function snapshotCanvasSurfacePresentation(
  change: CanvasSurfacePresentationChange,
): CanvasSurfacePresentationChange {
  const displayed =
    change.displayed.type === 'empty'
      ? Object.freeze({ ...change.displayed })
      : Object.freeze({
          ...change.displayed,
          viewport: snapshotCanvasSurfaceViewport(change.displayed.viewport),
        });
  return Object.freeze({ ...change, address: Object.freeze({ ...change.address }), displayed });
}

/** Require accepted preview observations to identify the exact fitted pixels actually displayed. */
export function validCanvasSurfacePresentation(change: CanvasSurfacePresentationChange): boolean {
  if (change.displayed.type === 'preview' && change.displayed.viewport.mode !== 'fit') return false;
  return (
    change.status !== 'presented' ||
    (change.displayed.type === 'preview' &&
      change.displayed.token === change.token &&
      change.displayed.leaseRevision === change.leaseRevision &&
      change.displayed.viewport.mode === 'fit')
  );
}
