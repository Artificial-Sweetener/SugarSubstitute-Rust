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
/** Keep the native display-only context aligned with the shared canvas schema. */

import { readFileSync } from 'node:fs';
import { canvasBindings } from '../../../scripts/integrations/generate-canvas-contract.js';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';

const vectors = JSON.parse(
  readFileSync('tests/editor_integrations/support/canvas_context_vectors.json', 'utf8'),
) as {
  request: unknown;
  available: unknown;
  unavailable: unknown;
  cancelled: unknown;
  invalidRequests: unknown[];
};

test('the display-only context and refusal use the same shared schema as clients', () => {
  expect(isCanvasMessage(vectors.request)).toBe(true);
  expect(isCanvasMessage(vectors.available)).toBe(true);
  expect(isCanvasMessage(vectors.unavailable)).toBe(true);
  expect(isCanvasMessage(vectors.cancelled)).toBe(true);
  for (const request of vectors.invalidRequests) expect(isCanvasMessage(request)).toBe(false);
});

test('the Rust context bindings are regenerated from the current canvas schema', async () => {
  const schema = readFileSync('integrations/contracts/canvas.schema.json', 'utf8');
  const generated = await canvasBindings(schema);
  expect(generated['integrations/contracts/CanvasNativeContext.generated.rs']).toBe(
    readFileSync('integrations/contracts/CanvasNativeContext.generated.rs', 'utf8').replaceAll(
      '\r\n',
      '\n',
    ),
  );
});

test('context generation rejects a schema that no longer permits a document-free context', async () => {
  const schema = JSON.parse(readFileSync('integrations/contracts/canvas.schema.json', 'utf8')) as {
    $defs: { CanvasContext: { properties: { document: unknown } } };
  };
  schema.$defs.CanvasContext.properties.document = { $ref: '#/$defs/CanvasDocument' };
  await expect(canvasBindings(JSON.stringify(schema))).rejects.toThrow('Canvas context schema');
});

test('native cancellation retains the existing error schema and its exact message bounds', async () => {
  const schema = JSON.parse(readFileSync('integrations/contracts/canvas.schema.json', 'utf8')) as {
    $defs: { CanvasFailure: { properties: { message: Record<string, unknown> } } };
  };
  schema.$defs.CanvasFailure.properties.message.maxLength = 21;
  const generated = await canvasBindings(JSON.stringify(schema));
  const rust = generated['integrations/contracts/CanvasNativeContext.generated.rs'];
  expect(rust).toContain('pub(super) struct CanvasErrorReply');
  expect(rust).toContain('pub(super) failure: CanvasFailure');
  expect(rust).toContain('self.message.encode_utf16().count() <= 21');
  schema.$defs.CanvasFailure.properties.message.minLength = 1;
  await expect(canvasBindings(JSON.stringify(schema))).rejects.toThrow(
    'Unsupported Rust contract schema keyword',
  );
});
