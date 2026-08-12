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
/** Own asynchronous SugarCubes extension setup orchestration. */

import type { SugarCubesUI } from '../SugarCubesUI.js';
import { CubeBlueprintPersistenceGuard } from '../affordance/CubeBlueprintPersistenceGuard.js';
import type { ComfyCubeOutputEventBridge } from './execution/ComfyCubeOutputEventBridge.js';
import type { ComfyCubeOutputHistoryAdapter } from './execution/ComfyCubeOutputHistoryAdapter.js';
import type { ComfyPromptQueueBridge } from './execution/ComfyPromptQueueBridge.js';
import type { SugarCubesSidebarHostAdapter } from '../sidebar/SugarCubesSidebarHostAdapter.js';
import type { HostSettingsController } from '../settings/HostSettingsController.js';
import type { ComfyApplication } from '../types/graph.js';

interface SetupLogger {
  error(...values: unknown[]): void;
  warn(...values: unknown[]): void;
}

interface CubeExtensionSetupLifecycleOptions {
  api: object;
  app: ComfyApplication | null;
  ui: SugarCubesUI;
  sidebar: SugarCubesSidebarHostAdapter;
  settings: HostSettingsController;
  promptQueue: ComfyPromptQueueBridge;
  outputEvents: ComfyCubeOutputEventBridge;
  outputHistory: ComfyCubeOutputHistoryAdapter;
  logger: SetupLogger;
}

/** Install host bridges and schedule initial graph-owned refreshes. */
export class CubeExtensionSetupLifecycle {
  #blueprintPersistenceGuard: CubeBlueprintPersistenceGuard | null = null;

  constructor(private readonly options: CubeExtensionSetupLifecycleOptions) {}

  /** Install every SugarCubes host integration in established order. */
  async setup(): Promise<void> {
    const options = this.options;
    try {
      options.sidebar.register();
      options.settings.register();
      await options.ui.setup();
      if (typeof Reflect.get(options.api, 'storeUserData') === 'function') {
        this.#blueprintPersistenceGuard ??= new CubeBlueprintPersistenceGuard(options.api);
        this.#blueprintPersistenceGuard.install();
      } else {
        options.logger.warn('SugarCubes: Comfy Blueprint persistence guard is unavailable.');
      }
      options.promptQueue.install();
      options.outputEvents.install();
      options.outputHistory.install();
      await options.outputHistory.hydrateRecent();
      await options.settings.refresh({ checkForUpdates: false });
      const graph = options.app?.canvas?.graph ?? options.app?.graph;
      options.ui.overlayManager.proximity.refreshOverlayState({
        recompute: true,
        ...(graph ? { graph } : {}),
      });
      options.settings.refreshUi();
      options.ui.instanceManager.scheduleRefresh({ ...(graph ? { graph } : {}), reason: 'setup' });
      options.ui.dirtyManager.requestRefresh({ ...(graph ? { graph } : {}), reason: 'setup' });
    } catch (error: unknown) {
      options.logger.error('SugarCubes: setup failed', error);
      throw error;
    }
  }
}
