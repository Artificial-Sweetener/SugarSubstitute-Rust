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
/** Preserve the browser's single custom-element registry across isolated entrypoint tests. */
import { jest } from '@jest/globals';
import * as spectrumRuntime from '../../../frontend/comfyui/ui/surface/appearance/SpectrumThemeRuntime.js';

/** Reuse real Adobe controls when Jest reloads source or compiled application entrypoints. */
export function preserveAppearanceRuntime(): void {
  jest.unstable_mockModule(
    '../../../frontend/comfyui/ui/surface/appearance/SpectrumThemeRuntime.js',
    () => spectrumRuntime,
  );
  jest.unstable_mockModule(
    '../../../web/comfyui/ui/surface/appearance/SpectrumThemeRuntime.js',
    () => spectrumRuntime,
  );
}
