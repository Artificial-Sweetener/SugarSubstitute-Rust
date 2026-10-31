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
/** Generate canvas bindings through the common contract compiler. */
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { contractBindings } from './ContractBindings.js';
import { rustContractBindings } from './RustContractBindings.js';

/** Retain only the display-only native context supported by the desktop host. */
function nativeContextDefinitions(definitions: Record<string, unknown>): Record<string, unknown> {
  const object = (value: unknown): Record<string, unknown> => {
    if (!value || typeof value !== 'object' || Array.isArray(value))
      throw new Error('Invalid canvas context schema.');
    return value as Record<string, unknown>;
  };
  const capabilities = object(definitions.CanvasCapabilities).oneOf;
  const descriptor = object(definitions.CanvasDescriptor);
  const context = object(definitions.CanvasContext);
  const reply = object(definitions.CanvasReadReply);
  const descriptorProperties = object(descriptor.properties);
  const contextProperties = object(context.properties);
  const replyProperties = object(reply.properties);
  const documents = object(contextProperties.document).oneOf;
  if (
    !Array.isArray(capabilities) ||
    capabilities.length !== 2 ||
    !Array.isArray(documents) ||
    documents.length !== 2 ||
    JSON.stringify(documents) !==
      JSON.stringify([{ $ref: '#/$defs/CanvasDocument' }, { const: null }]) ||
    JSON.stringify(descriptorProperties.capabilities) !==
      JSON.stringify({ $ref: '#/$defs/CanvasCapabilities' }) ||
    JSON.stringify(contextProperties.canvas) !==
      JSON.stringify({ $ref: '#/$defs/CanvasDescriptor' }) ||
    JSON.stringify(replyProperties.context) !== JSON.stringify({ $ref: '#/$defs/CanvasContext' })
  )
    throw new Error('Canvas context schema no longer supports the native display-only form.');
  return {
    CanvasReadRequest: definitions.CanvasReadRequest,
    CanvasFailure: definitions.CanvasFailure,
    CanvasErrorReply: definitions.CanvasErrorReply,
    CanvasNativeCapabilities: capabilities[0],
    CanvasNativeDescriptor: {
      ...descriptor,
      properties: {
        ...descriptorProperties,
        capabilities: { $ref: '#/$defs/CanvasNativeCapabilities' },
      },
    },
    CanvasNativeContext: {
      ...context,
      properties: {
        ...contextProperties,
        canvas: { $ref: '#/$defs/CanvasNativeDescriptor' },
        document: { const: null },
      },
    },
    CanvasNativeReadReply: {
      ...reply,
      properties: { ...replyProperties, context: { $ref: '#/$defs/CanvasNativeContext' } },
    },
  };
}
/** Preserve the public canvas build entry point while sharing schema policy. */
export async function canvasBindings(
  schemaText: string,
): Promise<Readonly<Record<string, string>>> {
  const schema = JSON.parse(schemaText) as {
    $defs: Record<string, unknown>;
    oneOf: readonly { $ref: string }[];
  };
  const surfaceNames = [
    'CanvasSurfaceCapabilities',
    'CanvasSurfaceCapabilitiesRequest',
    'CanvasSurfaceCapabilitiesReply',
    'CanvasSurfaceAddress',
    'CanvasSurfaceFileReference',
    'CanvasSurfacePoint',
    'CanvasSurfaceSize',
    'CanvasSurfaceClip',
    'CanvasSurfaceOcclusion',
    'CanvasSurfaceGeometry',
    'CanvasSurfacePrepareRequest',
    'CanvasSurfaceGeometryRequest',
    'CanvasSurfaceComparison',
    'CanvasSurfaceSelectRequest',
    'CanvasSurfaceOverviewRect',
    'CanvasSurfaceOverviewCell',
    'CanvasSurfaceOverviewRequest',
    'CanvasSurfaceCommitRequest',
    'CanvasSurfaceReleaseRequest',
    'CanvasSurfaceCancelRequest',
    'CanvasSurfaceReply',
    'CanvasSurfaceViewportCommand',
    'CanvasSurfaceImageDimensions',
    'CanvasSurfaceViewportState',
    'CanvasSurfaceViewportRequest',
    'CanvasSurfaceViewportReply',
    'CanvasSurfacePreviewIdentity',
    'CanvasSurfacePreviewBeginRequest',
    'CanvasSurfacePreviewChunkRequest',
    'CanvasSurfacePreviewEndRequest',
    'CanvasSurfacePreviewWithdrawRequest',
    'CanvasSurfacePresentationRequest',
    'CanvasSurfacePresentationReply',
    'CanvasSurfacePresentationDisplay',
    'CanvasSurfacePresentationChange',
    'CanvasSurfacePresentationChanged',
  ];
  for (const name of surfaceNames) {
    if (!(name in schema.$defs)) throw new Error(`Missing canvas surface definition: ${name}`);
  }
  const surface = Object.fromEntries(surfaceNames.map((name) => [name, schema.$defs[name]]));
  const surfaceMessages = schema.oneOf.filter(({ $ref }) =>
    surfaceNames.some((name) => $ref === `#/$defs/${name}`),
  );
  if (surfaceMessages.length !== 18) throw new Error('Canvas surface message union changed.');
  const ordinaryDefinitions = Object.fromEntries(
    Object.entries(schema.$defs).filter(([name]) => !surfaceNames.includes(name)),
  );
  const ordinaryMessages = schema.oneOf.filter(({ $ref }) =>
    surfaceMessages.every((message) => message.$ref !== $ref),
  );
  const ordinary = await contractBindings(
    JSON.stringify({
      $defs: ordinaryDefinitions,
      oneOf: [...ordinaryMessages, { $ref: '#/$defs/CanvasSurfaceMessage' }],
    }),
    {
      union: 'CanvasMessage',
      schema: 'canvas.schema.json',
      generator: 'scripts/integrations/generate-canvas-contract.ts',
      types: 'integrations/contracts/CanvasProtocol.generated.ts',
      validation: 'integrations/shared/CanvasValidation.generated.ts',
      importPath: '../contracts/CanvasProtocol.generated.js',
      recordImport: './ContractRecord.js',
      description: 'every version-three canvas message',
      external: {
        name: 'CanvasSurfaceMessage',
        typesImport: './CanvasSurfaceProtocol.generated.js',
        validationImport: './CanvasSurfaceValidation.generated.js',
      },
    },
  );
  const surfaces = await contractBindings(
    JSON.stringify({ $defs: surface, oneOf: surfaceMessages }),
    {
      union: 'CanvasSurfaceMessage',
      schema: 'canvas.schema.json',
      generator: 'scripts/integrations/generate-canvas-contract.ts',
      types: 'integrations/contracts/CanvasSurfaceProtocol.generated.ts',
      validation: 'integrations/shared/CanvasSurfaceValidation.generated.ts',
      importPath: '../contracts/CanvasSurfaceProtocol.generated.js',
      recordImport: './ContractRecord.js',
      description: 'the shared canvas surface messages',
      validationModules: [
        {
          file: 'integrations/shared/CanvasSurfacePreviewValidation.generated.ts',
          definitions: [
            'CanvasSurfacePreviewIdentity',
            'CanvasSurfacePreviewBeginRequest',
            'CanvasSurfacePreviewChunkRequest',
            'CanvasSurfacePreviewEndRequest',
            'CanvasSurfacePreviewWithdrawRequest',
            'CanvasSurfacePresentationRequest',
            'CanvasSurfacePresentationReply',
            'CanvasSurfacePresentationDisplay',
            'CanvasSurfacePresentationChange',
            'CanvasSurfacePresentationChanged',
          ],
        },
        {
          file: 'integrations/shared/CanvasSurfaceGeometryValidation.generated.ts',
          definitions: [
            'CanvasSurfaceAddress',
            'CanvasSurfaceFileReference',
            'CanvasSurfacePoint',
            'CanvasSurfaceSize',
            'CanvasSurfaceClip',
            'CanvasSurfaceOcclusion',
            'CanvasSurfaceGeometry',
          ],
        },
        {
          file: 'integrations/shared/CanvasSurfaceOverviewValidation.generated.ts',
          definitions: [
            'CanvasSurfaceOverviewRect',
            'CanvasSurfaceOverviewCell',
            'CanvasSurfaceOverviewRequest',
          ],
        },
        {
          file: 'integrations/shared/CanvasSurfaceViewportValidation.generated.ts',
          definitions: [
            'CanvasSurfaceViewportCommand',
            'CanvasSurfaceImageDimensions',
            'CanvasSurfaceViewportState',
            'CanvasSurfaceViewportRequest',
            'CanvasSurfaceViewportReply',
          ],
        },
      ],
    },
  );
  return {
    ...ordinary,
    ...surfaces,
    'integrations/contracts/CanvasSurfaceProtocol.generated.rs': rustContractBindings(
      surface,
      schemaText,
      {
        schema: 'canvas.schema.json',
        generator: 'scripts/integrations/generate-canvas-contract.ts',
        prefix: 'CanvasSurface',
      },
    ),
    'integrations/contracts/CanvasNativeContext.generated.rs': rustContractBindings(
      nativeContextDefinitions(schema.$defs),
      schemaText,
      {
        schema: 'canvas.schema.json',
        generator: 'scripts/integrations/generate-canvas-contract.ts',
        prefix: 'Canvas',
      },
    ),
  };
}
/** Generate or check bindings without modifying application or host state. */
async function main(): Promise<void> {
  const root = path.resolve(import.meta.dirname, '../..');
  const sources = await canvasBindings(
    readFileSync(path.join(root, 'integrations/contracts/canvas.schema.json'), 'utf8'),
  );
  for (const [relative, source] of Object.entries(sources)) {
    const file = path.join(root, relative);
    if (process.argv.includes('--check')) {
      if (readFileSync(file, 'utf8').replaceAll('\r\n', '\n') !== source)
        throw new Error(`Regenerate ${relative}.`);
    } else writeFileSync(file, source);
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href)
  main().catch((error: unknown) => {
    console.error(error);
    process.exitCode = 1;
  });
