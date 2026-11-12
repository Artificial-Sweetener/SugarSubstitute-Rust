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
/** Keep the native host's Rust wire module aligned with the shared canvas schema. */

import { readFileSync } from 'node:fs';
import { canvasBindings } from '../../../scripts/integrations/generate-canvas-contract.js';
import { isCanvasMessage } from '../../../integrations/shared/CanvasValidation.generated.js';

const NATIVE = 'integrations/contracts/CanvasNativeProtocol.generated.rs';
const vectors = JSON.parse(
  readFileSync('integrations/contracts/canvas_context_vectors.json', 'utf8'),
) as {
  request: unknown;
  available: unknown;
  unavailable: unknown;
  cancelled: unknown;
  invalidRequests: unknown[];
};
/** Expose the mutable members these tests alter in a schema definition. */
interface Definition {
  properties: Record<string, unknown>;
  oneOf?: unknown[];
  required?: string[];
}

/** Read a mutable copy of the authoritative schema. */
function schema(): { $defs: Record<string, Definition> } {
  return JSON.parse(readFileSync('integrations/contracts/canvas.schema.json', 'utf8')) as {
    $defs: Record<string, Definition>;
  };
}

/** Select one definition, failing the test when the schema no longer declares it. */
function definition(value: { $defs: Record<string, Definition> }, name: string): Definition {
  const node = value.$defs[name];
  if (!node) throw new Error(`Missing ${name} schema.`);
  return node;
}

/** Generate the native module from an authored or deliberately altered schema. */
async function native(value: unknown): Promise<string> {
  return (await canvasBindings(JSON.stringify(value)))[NATIVE] ?? '';
}

test('the display-only context and refusal use the same shared schema as clients', () => {
  expect(isCanvasMessage(vectors.request)).toBe(true);
  expect(isCanvasMessage(vectors.available)).toBe(true);
  expect(isCanvasMessage(vectors.unavailable)).toBe(true);
  expect(isCanvasMessage(vectors.cancelled)).toBe(true);
  for (const request of vectors.invalidRequests) expect(isCanvasMessage(request)).toBe(false);
});

test('the Rust wire module is regenerated from the current canvas schema', async () => {
  const generated = await canvasBindings(
    readFileSync('integrations/contracts/canvas.schema.json', 'utf8'),
  );
  expect(generated[NATIVE]).toBe(readFileSync(NATIVE, 'utf8').replaceAll('\r\n', '\n'));
});

test('the native context carries the full capability union and a nullable document', async () => {
  const rust = await native(schema());
  expect(rust).toContain(
    '#[serde(untagged)]\npub(super) enum CanvasCapabilities {\n    Layer(CanvasLayerCapabilities),\n    Geometry(CanvasGeometryCapabilities),\n    Editing(CanvasEditingCapabilities),\n}',
  );
  expect(rust).toContain(
    '    #[serde(deserialize_with = "deserialize_required_option")]\n    pub(super) document: Option<CanvasDocument>,',
  );
  for (const message of [
    'CanvasReadRequest',
    'CanvasReadReply',
    'CanvasErrorReply',
    'CanvasSourceRequest',
    'CanvasSourceReply',
    'CanvasSourceChanged',
    'CanvasGeometryRequest',
    'CanvasGeometryReply',
    'CanvasEditRequest',
    'CanvasEditReply',
    'CanvasEditChanged',
    'CanvasWorkflowRequest',
    'CanvasWorkflowReply',
  ])
    expect(rust).toContain(`pub(super) struct ${message} {`);
  // Referenced unions are flattened into one internally tagged enum per wire value.
  expect(rust).toContain('    #[serde(rename = "watch")]\n    Watch { enabled: bool },');
  expect(rust).not.toContain('CanvasSourceQuery');
  expect(rust).not.toContain('CanvasInsert');
  expect(rust).toContain('pub(super) enum CanvasToolId {');
  expect(rust).toContain('Nudge { dx: i64, dy: i64 }');
  expect(rust).toContain('self.data.encode_utf16().count() <= 89478488');
});

test('generation rejects a schema that no longer permits a document-free context', async () => {
  const value = schema();
  definition(value, 'CanvasContext').properties.document = { $ref: '#/$defs/CanvasDocument' };
  await expect(native(value)).rejects.toThrow('Canvas context schema');
});

test.each(['CanvasLayerCapabilities', 'CanvasEditingCapabilities'])(
  'generation rejects a capability union without the %s form',
  async (name) => {
    const value = schema();
    const capabilities = definition(value, 'CanvasCapabilities');
    capabilities.oneOf = (capabilities.oneOf ?? []).filter(
      (branch) => JSON.stringify(branch) !== JSON.stringify({ $ref: `#/$defs/${name}` }),
    );
    await expect(native(value)).rejects.toThrow('Canvas context schema');
  },
);

test('generation rejects a layer-only form that gains another capability', async () => {
  const value = schema();
  const layer = definition(value, 'CanvasLayerCapabilities');
  layer.properties.geometry = { type: 'boolean' };
  layer.required = [...(layer.required ?? []), 'geometry'];
  await expect(native(value)).rejects.toThrow('Canvas context schema');
});

test('native cancellation retains the existing error schema and its exact message bounds', async () => {
  const value = schema();
  const message = definition(value, 'CanvasFailure').properties.message as Record<string, unknown>;
  message.maxLength = 21;
  const rust = await native(value);
  expect(rust).toContain('pub(super) struct CanvasErrorReply');
  expect(rust).toContain('pub(super) failure: CanvasFailure');
  expect(rust).toContain('self.message.encode_utf16().count() <= 21');
  message.minLength = 1;
  await expect(native(value)).rejects.toThrow('Unsupported Rust contract schema keyword');
});
