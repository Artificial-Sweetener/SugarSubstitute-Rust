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
/** Carry explicit collaborators for saving presentation without a global service locator. */
import type { ModalAdapter } from '../dialogs/ModalShell.js';
import type { OutputSavingConfiguration } from './OutputSavingConfiguration.js';
import type { OutputSavingCapabilitiesClient } from './OutputSavingCapabilitiesClient.js';
import type { SourceSavingControls } from './SourceSavingControls.js';

export interface OutputSavingFeature {
  readonly configuration: OutputSavingConfiguration;
  readonly capabilities: OutputSavingCapabilitiesClient;
  readonly controls: SourceSavingControls | null;
  readonly adapter: ModalAdapter;
}
