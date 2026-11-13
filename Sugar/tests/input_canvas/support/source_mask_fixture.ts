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
  CanvasInputSource,
  CanvasSourceQuery,
  CanvasSourceResult,
  CanvasPixelRaster,
} from '../../../integrations/contracts/CanvasProtocol.generated.js';
import { encodeCanvasRaster } from '../../../integrations/shared/CanvasRasterEncoding.js';
import { editMaskInput } from '../../../frontend/comfyui/ui/input_canvas/domain/MaskInputEditing.js';
import type { MaskEditingPort } from '../../../frontend/comfyui/ui/input_canvas/application/MaskEditingPort.js';
import { MaskInputPreparation } from '../../../frontend/comfyui/ui/input_canvas/application/MaskInputPreparation.js';
import { ComfyMaskInputPreparation } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyMaskInputPreparation.js';
import type { WorkflowPreparedInput } from '../../../frontend/comfyui/ui/execution/domain/WorkflowPreparation.js';
import { nativeCanvasChanges } from '../../../integrations/shared/CanvasChangeRelay.js';
import { CanvasSourceCapture } from '../../../frontend/comfyui/ui/input_canvas/application/CanvasSourceCapture.js';
import { CanvasSourcePreviews } from '../../../frontend/comfyui/ui/input_canvas/application/CanvasSourcePreviews.js';
import { InputSourcePreparation } from '../../../frontend/comfyui/ui/input_canvas/application/InputSourcePreparation.js';
import { LegacyMaskMigration } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/LegacyMaskMigration.js';
import { ComfyInputSourceDefaults } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputSourceDefaults.js';
import { ComfyInputPreparation } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputPreparation.js';
import { ComfyInputLoaders } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputLoaders.js';

import { ComfyInputReturnPreparation } from '../../../frontend/comfyui/ui/input_canvas/infrastructure/ComfyInputReturnPreparation.js';

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
export function sourceMaskFixture(observesChanges = true) {
  const changes = new Set<(change: CanvasSourceChange) => void>();
  const execute = jest.fn(
    async (query: CanvasSourceQuery): Promise<CanvasSourceResult> =>
      query.type === 'context'
        ? {
            observesChanges,
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
    changes: observesChanges
      ? nativeCanvasChanges((listener) => {
          changes.add(listener);
          return () => {
            changes.delete(listener);
          };
        })
      : null,
  };
  const stage = jest.fn(
    async (_pixels: CanvasPixelRaster) => `immutable-${String(stage.mock.calls.length)}.png`,
  );
  const report = jest.fn(),
    capture = new CanvasSourceCapture((id) => (id === 'editor' ? host : null));
  const assets = { stage },
    legacy = new LegacyMaskMigration(() => null, assets);
  const loaders = new ComfyInputLoaders();
  loaders.define({
    'SimpleSyrup.LoadMaskBatch': { output: ['MASK'] },
    LoadImageMask: { output: ['MASK'] },
  });
  const defaults = new ComfyInputSourceDefaults(
    () => host,
    legacy,
    () => false,
  );
  const sourcePreparation = new InputSourcePreparation(capture, assets);
  const previews = new CanvasSourcePreviews(capture, () => host, report, {
    loaded: () => null,
    holds: () => true,
  });
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
  const maskEditing: MaskEditingPort = {
    prepare: async (source, settings) => editMaskInput(source, settings),
  };
  const editingPairs = new Map<string, string>();
  const projectInputs = jest.fn(
    async (prompt: unknown, _assets: readonly WorkflowPreparedInput[]): Promise<unknown> => prompt,
  );
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
    maskEditing,
    editingPairs,
    projectInputs,
    preparation: new ComfyInputPreparation(
      loaders,
      sourcePreparation,
      defaults,
      new ComfyInputReturnPreparation(files, async () => ({
        canvasId: 'editor',
        document: sourceDocument,
      })),
      new ComfyMaskInputPreparation(
        files,
        assets,
        new MaskInputPreparation(maskEditing),
        () => new Map(editingPairs),
      ),
      { prepare: projectInputs },
    ),
    ports: {
      host: () => host,
      canvases: { editable: () => true, provisions: () => false },
      refreshPreviews: (sources: readonly CanvasInputSource[]) => previews.refreshSources(sources),
      files,
      maskEditing,
      legacy,
      report,
      verifyFile: async () => true,
      verifiedFile: () => true,
    },
  };
}
