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
/** Characterize graph-bound Cube runtime ownership across workflow loads. */

import { jest } from '@jest/globals';
import { ComfyCubeRuntimeLifecycle } from '../../../frontend/comfyui/ui/cube/ComfyCubeRuntimeLifecycle.js';
import type { ComfyCubeRuntime } from '../../../frontend/comfyui/ui/cube/ComfyCubeRuntime.js';
import { CubeNodeCatalog } from '../../../frontend/comfyui/ui/cube/node/CubeNodeCatalog.js';
import { CubeEditorContextResolver } from '../../../frontend/comfyui/ui/surface/CubeEditorContextResolver.js';

/** Build the smallest graph-bound runtime needed by the lifecycle owner. */
function createRuntime(dispose: () => void): ComfyCubeRuntime {
  const nodes = new CubeNodeCatalog();
  return {
    construction: {} as ComfyCubeRuntime['construction'],
    placement: {} as ComfyCubeRuntime['placement'],
    authoring: {} as ComfyCubeRuntime['authoring'],
    nodes,
    cardReveal: {} as ComfyCubeRuntime['cardReveal'],
    versionAvailability: {} as ComfyCubeRuntime['versionAvailability'],
    versionSwitch: {} as ComfyCubeRuntime['versionSwitch'],
    graphScope: {} as ComfyCubeRuntime['graphScope'],
    graphInventory: {} as ComfyCubeRuntime['graphInventory'],
    hostPlacementGuard: {} as ComfyCubeRuntime['hostPlacementGuard'],
    contexts: new CubeEditorContextResolver(nodes),
    metadataHud: null,
    proximityEndpoints: { discover: () => ({ outputs: [], inputs: [] }) },
    proximityPresentation: { updateMatches: () => undefined },
    registerSubgraphs: () => [],
    restoreLegacy: () => ({ migrated: 0, connected: 0, warnings: [] }),
    detectLegacyBlueprints: () => 0,
    dispose,
  };
}

describe('ComfyCubeRuntimeLifecycle', () => {
  test('creates lazily and disposes before the next graph configuration', () => {
    const firstDispose = jest.fn();
    const secondDispose = jest.fn();
    const create = jest
      .fn<() => ComfyCubeRuntime>()
      .mockReturnValueOnce(createRuntime(firstDispose))
      .mockReturnValueOnce(createRuntime(secondDispose));
    const lifecycle = new ComfyCubeRuntimeLifecycle(create);

    expect(lifecycle.current()).toBeNull();
    expect(lifecycle.require()).toBe(lifecycle.require());
    expect(lifecycle.current()).not.toBeNull();
    expect(create).toHaveBeenCalledTimes(1);

    lifecycle.reset();
    expect(firstDispose).toHaveBeenCalledTimes(1);
    expect(lifecycle.current()).toBeNull();
    expect(lifecycle.require()).not.toBeNull();
    expect(create).toHaveBeenCalledTimes(2);

    lifecycle.reset();
    expect(secondDispose).toHaveBeenCalledTimes(1);
  });
});
