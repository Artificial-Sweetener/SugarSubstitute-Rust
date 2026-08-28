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
/** Compose the current Comfy picker adapters behind one focused integration. */

import type { CubeLibraryApi } from '../core/CubeLibraryApi.js';
import type { ComfyCubeRuntime } from '../cube/ComfyCubeRuntime.js';
import type { NodeRenderer } from '../geometry/RendererGeometryPolicy.js';
import { isRecord } from '../types/common.js';
import { ComfyCubePickerCreationAdapter } from './ComfyCubePickerCreationAdapter.js';
import type { CubePickerLiteGraphHost } from './ComfyCubePickerCreationAdapter.js';
import { ComfyCubePickerDefinitionAdapter } from './ComfyCubePickerDefinitionAdapter.js';
import type { CubePickerDefinitionHost } from './ComfyCubePickerDefinitionAdapter.js';
import { ComfyCubePickerResultPresenter } from './ComfyCubePickerResultPresenter.js';
import { CubePickerCatalogRegistry } from './CubePickerCatalogRegistry.js';
import { CubePickerHostIntegration } from './CubePickerHostIntegration.js';
import { CubePickerPlacementAdapter } from './CubePickerPlacementAdapter.js';
import { CubeConnectionTypePolicy } from '../cube/connection/CubeConnectionTypePolicy.js';
import { CubeAddCandidateCatalog } from './CubeAddCandidateCatalog.js';
import { CubePickerInsertAfterService } from './CubePickerInsertAfterService.js';
import { ComfyCubeAddMenuPresenter } from './ComfyCubeAddMenuPresenter.js';
import { CubeAddMenuController } from './CubeAddMenuController.js';
import { createComfyRendererModeChangeSource } from '../core/ComfyRendererMode.js';

export interface ComfyCubePickerCompositionOptions {
  api: CubeLibraryApi;
  app: unknown;
  document: Document;
  getLiteGraph(): unknown;
  getNodeRenderer(): NodeRenderer | undefined;
  getRuntime(): ComfyCubeRuntime;
  logger: Pick<Console, 'debug' | 'error' | 'warn'>;
  reportError(summary: string, detail: string): void;
  isProximityStrict(): boolean;
}

/** Build one picker integration while isolating all dynamic Comfy capability checks. */
export function createComfyCubePickerIntegration(
  options: ComfyCubePickerCompositionOptions,
): CubePickerHostIntegration {
  const app = readDefinitionHost(options.app);
  const getLiteGraph = (): CubePickerLiteGraphHost | null =>
    readLiteGraphHost(options.getLiteGraph());
  const registry = new CubePickerCatalogRegistry({ api: options.api, logger: options.logger });
  const placement = new CubePickerPlacementAdapter({
    registry,
    getRuntime: options.getRuntime,
    getLiteGraph,
    getNodeRenderer: options.getNodeRenderer,
    logger: options.logger,
  });
  const creation = new ComfyCubePickerCreationAdapter({
    getLiteGraph,
    placement,
    reportError: options.reportError,
  });
  const definitions = new ComfyCubePickerDefinitionAdapter({
    registry,
    app,
    liteGraph: {
      unregisterNodeType(type) {
        const liteGraph = getLiteGraph();
        const unregister = liteGraph ? Reflect.get(liteGraph, 'unregisterNodeType') : null;
        if (typeof unregister === 'function') unregister.call(liteGraph, type);
      },
    },
  });
  const results = new ComfyCubePickerResultPresenter({
    document: options.document,
    definitions: () => registry.definitions(),
  });
  const getConnectionLiteGraph = () => {
    const host = getLiteGraph();
    if (!host) return null;
    const isValidConnection = host.isValidConnection;
    return typeof isValidConnection === 'function'
      ? {
          isValidConnection: (outputType: unknown, inputType: unknown) =>
            Boolean(isValidConnection.call(host, outputType, inputType)),
        }
      : {};
  };
  const candidates = new CubeAddCandidateCatalog({
    compatibility: new CubeConnectionTypePolicy({
      getLiteGraph: getConnectionLiteGraph,
      logger: options.logger,
    }),
    strict: options.isProximityStrict,
  });
  const insertion = new CubePickerInsertAfterService({
    placement,
    getRuntime: options.getRuntime,
  });
  const addMenu = new CubeAddMenuController({
    registry,
    candidates,
    insertion,
    presenter: new ComfyCubeAddMenuPresenter(options.document),
    getRuntime: options.getRuntime,
    reportError: options.reportError,
  });
  return new CubePickerHostIntegration({
    definitions,
    creation,
    results,
    addMenu,
    rendererChanges: createComfyRendererModeChangeSource(options.app),
    logger: options.logger,
    reportError: options.reportError,
  });
}

/** Validate only the Comfy application methods used for dynamic reconciliation. */
function readDefinitionHost(value: unknown): CubePickerDefinitionHost {
  if (!isRecord(value)) return {};
  const host: CubePickerDefinitionHost = {};
  const registerNodeDef = value.registerNodeDef;
  if (typeof registerNodeDef === 'function') {
    host.registerNodeDef = (type, definition) => registerNodeDef.call(value, type, definition);
  }
  const reloadNodeDefs = value.reloadNodeDefs;
  if (typeof reloadNodeDefs === 'function') {
    host.reloadNodeDefs = () => reloadNodeDefs.call(value);
  }
  return host;
}

/** Validate the mutable LiteGraph factory surface at its adapter boundary. */
function readLiteGraphHost(value: unknown): CubePickerLiteGraphHost | null {
  if (!isRecord(value) || typeof value.createNode !== 'function') return null;
  return value as CubePickerLiteGraphHost;
}
