/** @jest-environment node */
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
/** Validate bounded preview wire values and canonical byte admission independently of native decoding. */
import { readFileSync } from 'node:fs';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';
import { encodeCanvasBytes } from '../../../integrations/shared/CanvasByteEncoding.js';
import { MAX_CANVAS_IMAGE_BYTES } from '../../../integrations/shared/CanvasImagePolicy.js';
import {
  CANVAS_PREVIEW_CHUNK_BYTES,
  CANVAS_PREVIEW_LEASE_MILLISECONDS,
  canvasPreviewMessageFits,
  decodeCanvasPreviewChunk,
  validCanvasPreviewCommand,
  validCanvasSurfacePresentation,
} from '../../../integrations/shared/CanvasSurfacePreview.js';
import { isCanvasSurfacePresentationChange } from '../../../integrations/shared/CanvasSurfaceValidation.generated.js';
import type { CanvasSurfaceCommand } from '../../../integrations/contracts/CanvasSurface.js';
import type { CanvasMessage } from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { canvasBindings } from '../../../scripts/integrations/generate-canvas-contract.js';

const address = {
  sessionId: 'canvas:workflow:one',
  viewId: 'view:one',
  windowId: 'window:one',
  attachmentId: 'attachment:one',
  generation: 1,
} as const;
const preview = {
  generation: 0,
  promptId: 'prompt:one',
  sourceId: 'source:10:20',
  runOrder: 1,
  sequence: 1,
  token: 'preview:1',
} as const;
const begin: CanvasSurfaceCommand = {
  type: 'surface.preview.begin',
  address,
  revision: 3,
  preview,
  byteLength: 3,
  format: 'jpeg',
};
const vectors: {
  validRequests: unknown[];
  validReplies: unknown[];
  validChanges: unknown[];
  invalidMessages: unknown[];
} = JSON.parse(
  readFileSync(new URL('../support/canvas_surface_preview_vectors.json', import.meta.url), 'utf8'),
);

test('shares strict attribution and canonical byte admission vectors beyond wire shape validation', () => {
  const admission: {
    validCommands: unknown[];
    invalidCommands: unknown[];
    validChanges: unknown[];
    invalidChanges: unknown[];
  } = JSON.parse(
    readFileSync(
      new URL('../support/canvas_surface_preview_admission_vectors.json', import.meta.url),
      'utf8',
    ),
  );
  for (const message of admission.validCommands) {
    expect(isCanvasMessage(message)).toBe(true);
    if (!isCanvasMessage(message) || !('address' in message))
      throw new Error('Malformed admission fixture.');
    const accepted = message as Extract<
      CanvasMessage,
      { type: 'surface.preview.begin' | 'surface.preview.chunk' }
    >;
    expect(validCanvasPreviewCommand(accepted)).toBe(true);
  }
  for (const message of admission.invalidCommands) {
    if (
      isCanvasMessage(message) &&
      (message.type === 'surface.preview.begin' || message.type === 'surface.preview.chunk')
    )
      expect(validCanvasPreviewCommand(message)).toBe(false);
  }
  for (const change of admission.validChanges) {
    if (!isCanvasSurfacePresentationChange(change))
      throw new Error('Invalid presentation fixture.');
    expect(validCanvasSurfacePresentation(change)).toBe(true);
  }
  for (const change of admission.invalidChanges) {
    if (!isCanvasSurfacePresentationChange(change))
      throw new Error('Invalid presentation fixture.');
    expect(validCanvasSurfacePresentation(change)).toBe(false);
  }
});

test('accepts bounded preview custody operations and opt-in presentation observations', () => {
  for (const message of [
    ...vectors.validRequests,
    ...vectors.validReplies,
    ...vectors.validChanges,
  ])
    expect(isCanvasMessage(message)).toBe(true);
});

test('rejects malformed, extended and out-of-budget preview messages', () => {
  for (const message of vectors.invalidMessages) expect(isCanvasMessage(message)).toBe(false);
});

test('enforces canonical bounded chunks and preserves the original fixed deadline contract', () => {
  expect(CANVAS_PREVIEW_LEASE_MILLISECONDS).toBe(30000);
  expect(CANVAS_PREVIEW_CHUNK_BYTES).toBe(8192);
  expect(MAX_CANVAS_IMAGE_BYTES).toBe(67108864);
  const bytes = new Uint8Array(CANVAS_PREVIEW_CHUNK_BYTES);
  const encoded = encodeCanvasBytes(bytes);
  expect(encoded).toHaveLength(10924);
  expect(decodeCanvasPreviewChunk(encoded, 0, bytes.length)).toEqual(bytes);
  expect(() => decodeCanvasPreviewChunk('/x==', 0, 1)).toThrow();
  expect(() => decodeCanvasPreviewChunk('AQID', 1, 4)).toThrow();
  expect(() => decodeCanvasPreviewChunk('AQID', 0, 8192)).toThrow();
  expect(() => decodeCanvasPreviewChunk('AQID', 0, MAX_CANVAS_IMAGE_BYTES + 1)).toThrow();
  expect(validCanvasPreviewCommand({ ...begin, preview: { ...preview, token: 'preview:2' } })).toBe(
    false,
  );
});

test('fits maximum escaped identities and a full chunk inside the public JSON transport budget', () => {
  for (const character of ['\\', '"', '😀']) {
    const largestAddress = {
      sessionId: 'canvas:' + character.repeat(249),
      viewId: 'view:' + character.repeat(123),
      windowId: 'window:' + character.repeat(121),
      attachmentId: 'attachment:' + character.repeat(117),
      generation: Number.MAX_SAFE_INTEGER,
    };
    const message: CanvasMessage = {
      protocol: 'sugarcubes.canvas',
      version: 3,
      session: 's'.repeat(80),
      requestId: 2147483647,
      type: 'surface.preview.chunk',
      address: largestAddress,
      revision: Number.MAX_SAFE_INTEGER,
      token: 'preview:9007199254740991',
      leaseRevision: Number.MAX_SAFE_INTEGER,
      offset: MAX_CANVAS_IMAGE_BYTES - CANVAS_PREVIEW_CHUNK_BYTES,
      data: encodeCanvasBytes(new Uint8Array(CANVAS_PREVIEW_CHUNK_BYTES)),
    };
    expect(isCanvasMessage(message)).toBe(true);
    expect(canvasPreviewMessageFits(message)).toBe(true);
  }
});

test('generates only the reviewed bounded base64 vocabulary and rejects widened or ambiguous schemas', async () => {
  const text = readFileSync('integrations/contracts/canvas.schema.json', 'utf8');
  const schema = JSON.parse(text) as {
    $defs: {
      CanvasSurfacePreviewChunkRequest: {
        properties: { data: { maxLength: number; pattern: string } };
      };
    };
  };
  const generated = await canvasBindings(text);
  expect(generated['integrations/contracts/CanvasSurfaceProtocol.generated.rs']).toContain(
    'valid_bounded_base64(&self.data, 10924)',
  );
  expect(generated['integrations/contracts/CanvasSurfaceProtocol.generated.rs']).toContain(
    '(1..=9007199254740991).contains(selection_revision) && viewport.valid()',
  );
  schema.$defs.CanvasSurfacePreviewChunkRequest.properties.data.maxLength = 10925;
  await expect(canvasBindings(JSON.stringify(schema))).rejects.toThrow();
  schema.$defs.CanvasSurfacePreviewChunkRequest.properties.data.maxLength = 10924;
  schema.$defs.CanvasSurfacePreviewChunkRequest.properties.data.pattern = '.*';
  await expect(canvasBindings(JSON.stringify(schema))).rejects.toThrow();
});
