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
/** Supply attributable synthetic mixed outputs and a controlled external model-viewer boundary. */
import { jest } from '@jest/globals';
import type { WorkflowOutputItem } from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import type {
  CanvasModelRuntime,
  CanvasModelViewer,
} from '../../../frontend/comfyui/ui/output_canvas/infrastructure/ComfyCanvasModelRuntime.js';
import { canvasCatalogFixture } from './CanvasCatalogFixture.js';
import { canvasSnapshot } from './CanvasPortFixture.js';
import { canvasDisplayPlan } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasDisplayPlan.js';

/** Retain file identity and batch positions across image, video and model presentation. */
export function mixedMediaFixture() {
  const base = canvasCatalogFixture();
  const entry = base.entries[0]!;
  const items = (['image', 'video', 'model', 'animated'] as const).map(
    (mediaKind, batchIndex): WorkflowOutputItem => ({
      ...entry.items[0]!,
      id: `result:mixed-${batchIndex}`,
      batchIndex,
      mediaKind,
      file: {
        filename: ['still.png', 'clip.webm', 'triangle.glb', 'animation.gif'][batchIndex]!,
        subfolder: 'synthetic',
        type: 'temp',
      },
    }),
  );
  const catalog = {
    ...base,
    entries: [{ ...entry, items, summary: { ...entry.summary, batchCount: 4 } }],
  };
  const plan = (index: number, overview = false) => {
    const item = items[index]!;
    const view = {
      ...canvasSnapshot('mixed').view,
      sessionId: catalog.sessionId,
      mode: overview ? ('overview' as const) : ('image' as const),
      selection: { sourceId: item.provenance.sourceId, batchIndex: index },
    };
    return canvasDisplayPlan(view, catalog, { primary: item, secondary: null }, null);
  };
  return { catalog, items, plan };
}

/** Delay only the external viewer so production lifecycle and DOM behavior remain real. */
export class CanvasModelRuntimeFixture implements CanvasModelRuntime {
  readonly requests: {
    signal: AbortSignal;
    resolve(viewer: CanvasModelViewer): void;
    reject(error: unknown): void;
  }[] = [];
  /** Capture one queued mount without allocating a GPU context in unit tests. */
  load(
    _container: HTMLElement,
    _url: string,
    _filename: string,
    signal: AbortSignal,
  ): Promise<CanvasModelViewer> {
    return new Promise((resolve, reject) => {
      this.requests.push({ signal, resolve, reject });
    });
  }
  /** Supply a host-shaped viewer whose disposal can be observed. */
  viewer(): CanvasModelViewer {
    return {
      resize: jest.fn(),
      capture: jest.fn<() => Promise<string>>().mockResolvedValue('data:image/png;base64,AA=='),
      camera: () => ({ zoom: 2 }),
      restore: jest.fn(),
      dispose: jest.fn(),
    };
  }
}
