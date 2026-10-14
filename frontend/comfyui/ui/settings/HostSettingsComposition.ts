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
import { OutputSavingConfiguration } from '../output_saving/OutputSavingConfiguration.js';
import { OutputSavingCapabilitiesClient } from '../output_saving/OutputSavingCapabilitiesClient.js';
import { SourceSavingControls } from '../output_saving/SourceSavingControls.js';
import { SourceSavingDialog } from '../output_saving/SourceSavingDialog.js';
import type { OutputSavingFeature } from '../output_saving/OutputSavingFeature.js';

export interface HostSettingsComposition extends HostSettingsController {
  readonly outputSaving: OutputSavingFeature;
}

/** Preserve the host-facing settings lifecycle while delegating each feature to its authoritative controller. */
export function createHostSettingsComposition(
  dependencies: Parameters<typeof createHostSettingsController>[0],
): HostSettingsComposition {
  const repository = createHostSettingsController(dependencies);
  const document = dependencies.adapter.getDocument();
  const configuration = new OutputSavingConfiguration(new OutputSavingClient(dependencies.cubeApi));
  const controls = document
    ? new SourceSavingControls(
        configuration,
        new SourceSavingDialog(document, dependencies.adapter),
      )
    : null;
  const outputSaving: OutputSavingFeature = {
    configuration,
    controls,
    capabilities: new OutputSavingCapabilitiesClient(dependencies.cubeApi),
    adapter: dependencies.adapter,
  };
  const saving = document
    ? new OutputSavingSettings(
        document,
        configuration,
        dependencies.adapter,
        (error) => {
          dependencies.logger.warn('SugarCubes output settings failed', { error });
          dependencies.pushToast(
            'error',
            'Output settings unavailable',
            dependencies.readErrorMessage(error),
          );
        },
        controls ? () => controls.open() : null,
      )
    : null;
  return {
    outputSaving,
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
