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
/** Build real attributable result sessions for canvas navigation proofs. */

import {
  workflowSessionId,
  outputSourceId,
  promptId,
} from '../../../frontend/comfyui/ui/execution/domain/ExecutionIdentity.js';
import type { WorkflowResultSession } from '../../../frontend/comfyui/ui/execution/domain/WorkflowResultSession.js';
import type {
  WorkflowOutputSource,
  WorkflowOutputItem,
} from '../../../frontend/comfyui/ui/execution/domain/WorkflowOutput.js';
import { canvasOutputCatalog } from '../../../frontend/comfyui/ui/output_canvas/application/CanvasOutputCatalog.js';
import type { CanvasCatalogSnapshot } from '../../../frontend/comfyui/ui/output_canvas/domain/CanvasCatalog.js';

/** Declare repeatable paths whose labels may intentionally collide. */
export function canvasSources(): readonly WorkflowOutputSource[] {
  return ['1', '7:23', '8:23'].map((path, order) => ({
    id: outputSourceId(path),
    executionPath: path,
    label: order ? 'Preview' : '<b>Output</b>',
    breadcrumb: order ? ['Nested'] : [],
    order,
  }));
}

/** Construct immutable-shaped lightweight files without depending on browser image decoding. */
export function canvasCatalogFixture(
  run = 1,
  generation = 0,
  sources = canvasSources(),
  key = 'canvas',
): CanvasCatalogSnapshot {
  const context = { sessionId: workflowSessionId(key), generation, title: key, path: '' };
  const outputs = sources.map((source) => {
    const provenance = {
      ...context,
      sourceId: source.id,
      executionPath: source.executionPath,
      runOrder: run,
      sourceOrder: source.order,
      revision: run,
      kind: 'execution' as const,
      promptId: promptId(String(run)),
    };
    const item: WorkflowOutputItem = {
      id: `result:${key}-${generation}-${source.id}-${run}`,
      provenance,
      batchIndex: 0,
      file: { filename: `${run}.png`, subfolder: '', type: 'output' },
      mediaKind: 'image',
      dimensions: { width: 3840, height: 2160 },
    };
    return { source, provenance, items: [item], batchCount: 3, text: [] };
  });
  const result: WorkflowResultSession = {
    context,
    revision: run,
    outputs,
    runOrder: run,
    runStatus: 'complete',
    preview: null,
    recovery: { state: 'idle', message: '' },
  };
  return canvasOutputCatalog(result, sources);
}
