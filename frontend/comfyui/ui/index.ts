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
/**
 * Own the SugarCubes UI orchestration layer in `frontend/comfyui/ui/index.js`.
 */

import { SugarCubesUI } from './SugarCubesUI.js';
import type { UnknownRecord } from './types/common.js';

let instance: SugarCubesUI | null = null;

interface SugarCubesUIFactoryOptions extends UnknownRecord {
  forceNew?: boolean;
}

/**
 * Create sugar cubes ui.
 */
export function createSugarCubesUI(options: SugarCubesUIFactoryOptions = {}): SugarCubesUI {
  return new SugarCubesUI(options);
}

/**
 * Get sugar cubes ui.
 */
export function getSugarCubesUI(options: SugarCubesUIFactoryOptions = {}): SugarCubesUI {
  const forceNew = options.forceNew === true;
  if (!instance || forceNew) {
    const { forceNew: _ignored, ...rest } = options;
    instance = new SugarCubesUI(rest);
  }
  return instance;
}

/**
 * Create public api.
 */
export function createPublicApi(uiInstance: SugarCubesUI) {
  if (!uiInstance) {
    throw new Error('SugarCubes UI instance required for public API.');
  }
  return Object.freeze({
    listCubes: uiInstance.listCubes.bind(uiInstance),
    previewCube: uiInstance.previewCube.bind(uiInstance),
    scheduleCubeInstanceRefresh: uiInstance.scheduleCubeInstanceRefresh.bind(uiInstance),
    scheduleCubeDirtyRefresh: uiInstance.scheduleCubeDirtyRefresh.bind(uiInstance),
    openLibrary: uiInstance.openLibrary.bind(uiInstance),
  });
}
