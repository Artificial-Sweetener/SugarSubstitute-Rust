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
/** Supply a capable peer and real mask-source owners behind deterministic native and file boundaries. */
import { jest } from '@jest/globals';
import type { CanvasSourceHost } from '../../../integrations/contracts/CanvasSources.js';
import type {
  CanvasSourceChange,
  CanvasSourceQuery,
  CanvasSourceResult,
  CanvasPixelRaster,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { encodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';
import { CanvasSourceCapture } from '../../../frontend/comfyui/ui/input_canvas/application/CanvasSourceCapture.js';
import { CanvasSourcePreviews } from '../../../frontend/comfyui/ui/input_canvas/application/CanvasSourcePreviews.js';
import { InputSourcePreparation } from '../../../frontend/comfyui/ui/input_canvas/application/InputSourcePreparation.js';
import { LegacyMaskMigration } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/LegacyMaskMigration.js';
import { ComfyInputSourceDefaults } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputSourceDefaults.js';
import { ComfyInputPreparation } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputPreparation.js';
import { ComfyInputLoaders } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputLoaders.js';

export const sourceDocument = { id: 'doc', geometryRevision: 'geometry', width: 3, height: 2 };
export const sourceCoverage = encodeCanvasRaster(
  3,
  2,
  'coverage8',
  Uint8Array.from([0, 255, 0, 127, 0, 255]),
);
export const sourceRgba = encodeCanvasRaster(
  3,
  2,
  'rgba8',
  Uint8Array.from([
    10, 20, 30, 0, 40, 50, 60, 127, 70, 80, 90, 255, 100, 110, 120, 127, 130, 140, 150, 255, 160,
    170, 180, 0,
  ]),
);

/** Keep capture and persistence real while replacing only external canvas and network boundaries. */
export function sourceMaskFixture() {
  const changes = new Set<(change: CanvasSourceChange) => void>();
  const execute = jest.fn(
    async (query: CanvasSourceQuery): Promise<CanvasSourceResult> =>
      query.type === 'context'
        ? {
            kind: 'context',
            supported: true,
            document: sourceDocument,
            revision: '1',
            targets: [
              'selection',
              'current-layer',
              'layer',
              'composition',
              'composition-from-layer',
            ],
            layers: [
              { id: 'a', name: 'First layer', parentId: null, depth: 0, visible: true },
              { id: 'b', name: 'Second layer', parentId: null, depth: 0, visible: true },
            ],
            currentLayerId: 'a',
          }
        : {
            kind: 'captured',
            snapshots: query.targets.map((target) => ({
              target,
              document: sourceDocument,
              revision: '1',
              pixels: target.kind === 'selection' ? sourceCoverage : sourceRgba,
            })),
          },
  );
  const host: CanvasSourceHost = {
    canvasId: 'editor',
    execute,
    subscribe: (listener) => {
      changes.add(listener);
      return () => {
        changes.delete(listener);
      };
    },
  };
  const stage = jest.fn(
    async (_pixels: CanvasPixelRaster) => `immutable-${String(stage.mock.calls.length)}.png`,
  );
  const report = jest.fn(),
    capture = new CanvasSourceCapture((id) => (id === 'editor' ? host : null));
  const assets = { stage },
    legacy = new LegacyMaskMigration(() => null, assets);
  const loaders = new ComfyInputLoaders();
  loaders.define({ 'SimpleSyrup.LoadMaskBatch': {}, LoadImageMask: {} });
  const defaults = new ComfyInputSourceDefaults(() => host, legacy);
  const sourcePreparation = new InputSourcePreparation(capture, assets);
  const previews = new CanvasSourcePreviews(capture, () => host, report);
  const files = {
    interpret: jest.fn(
      async (): Promise<CanvasPixelRaster> => ({
        format: 'coverage8' as const,
        width: sourceCoverage.width,
        height: sourceCoverage.height,
        data: sourceCoverage.data,
      }),
    ),
    load: jest.fn(
      async (): Promise<CanvasPixelRaster> => ({
        format: 'coverage8' as const,
        width: sourceCoverage.width,
        height: sourceCoverage.height,
        data: sourceCoverage.data,
      }),
    ),
  };
  return {
    host,
    execute,
    changes,
    stage,
    report,
    capture,
    legacy,
    loaders,
    defaults,
    sourcePreparation,
    previews,
    files,
    preparation: new ComfyInputPreparation(loaders, sourcePreparation, defaults),
    ports: {
      host: () => host,
      files,
      legacy,
      report,
      verifyFile: async () => true,
      verifiedFile: () => true,
    },
  };
}
