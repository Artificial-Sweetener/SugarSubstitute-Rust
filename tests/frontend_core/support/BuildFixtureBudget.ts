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
 * Bound every setup hook that bundles or packages source before its tests run, such as the
 * Photoshop contract suites' esbuild of native modules and builds of the UXP package.
 *
 * Bundling is CPU-bound and stretches with machine-wide load in a way the tests' observations
 * do not, so build setup shares this budget, as browser setup shares its own, while test bodies
 * keep their own limits.
 */
export const BUILD_FIXTURE_SETUP_TIMEOUT_MS = 60_000;
