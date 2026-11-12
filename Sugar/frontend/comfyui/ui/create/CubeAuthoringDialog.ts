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
/** Define the application-facing contract for Cube authoring confirmation. */

import type { CubeSaveDestination } from './CubeAuthoringIdentity.js';
import type { PersonalCubeIdentity } from './PersonalCubeIdentity.js';

export interface CubeAuthoringMetadataDraft {
  defaultAlias: string;
  description: string;
  supportedModels: string[];
  targetModel: string;
  destination: CubeSaveDestination['kind'];
}

export interface CubeAuthoringDestinationOption {
  action?: 'create-pack';
  destination?: CubeSaveDestination;
  detail?: string;
  key: string;
  label: string;
}

export interface CubeAuthoringCandidate {
  cubeId?: string;
  defaultAlias?: string;
  description?: string;
  destination?: CubeSaveDestination | CubeSaveDestination['kind'];
  inputCount?: number;
  markerIds?: unknown[];
  nodeIds?: unknown[];
  outputCount?: number;
  supportedModels?: string[];
  targetModel?: string;
  warnings?: unknown[];
}

export interface CubeAuthoringValues extends PersonalCubeIdentity {
  description: string;
  supportedModels: string[];
  targetModel: string;
  destination: CubeSaveDestination;
}

export interface CubeAuthoringDialogOptions {
  candidate?: CubeAuthoringCandidate | null;
  destinationLocked?: boolean;
  destinations?: readonly CubeAuthoringDestinationOption[];
  modelSuggestions?: readonly string[];
  onCreateDestination?: () => Promise<CubeAuthoringDestinationOption | null>;
  deriveIdentity?: (
    name: string,
    targetModel: string,
    destination: CubeSaveDestination,
  ) => PersonalCubeIdentity;
}

export interface CubeAuthoringDialog {
  openCubeAuthoring(options: CubeAuthoringDialogOptions): Promise<CubeAuthoringValues | null>;
}
