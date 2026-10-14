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
/** Compose independent repository and output-saving settings owners for the host lifecycle. */
import {
  createHostSettingsController,
  type HostSettingsController,
} from './HostSettingsController.js';
import { OutputSavingClient } from '../output_saving/OutputSavingClient.js';
import { OutputSavingSettings } from '../output_saving/OutputSavingSettings.js';

/** Preserve the host-facing settings lifecycle while delegating each feature to its authoritative controller. */
export function createHostSettingsComposition(
  dependencies: Parameters<typeof createHostSettingsController>[0],
): HostSettingsController {
  const repository = createHostSettingsController(dependencies);
  const document = dependencies.adapter.getDocument();
  const saving = document
    ? new OutputSavingSettings(
        document,
        new OutputSavingClient(dependencies.cubeApi),
        dependencies.adapter,
        (error) => {
          dependencies.logger.warn('SugarCubes output settings failed', { error });
          dependencies.pushToast(
            'error',
            'Output settings unavailable',
            dependencies.readErrorMessage(error),
          );
        },
      )
    : null;
  return {
    register: () => {
      repository.register();
      const manager = dependencies.appRef?.ui?.settings;
      if (manager) saving?.register(manager);
    },
    refresh: async (options) => {
      await Promise.all([repository.refresh(options), saving?.refresh()]);
    },
    refreshUi: () => {
      repository.refreshUi();
      saving?.show();
    },
  };
}
