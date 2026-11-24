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
 * The Python SugarSubstitute's Settings, page by page: its navigation order, page titles and
 * subtitles, its sections in order, and each row's title, description and icon
 * (`substitute/presentation/settings/`, recorded in the settings inventory). Rows SugarCubes adds
 * sit in the section that fits them. A row shows only once a feature gives it its controls.
 */
import type { SettingsPageLayout } from './SettingsLayoutRows.js';
import { aboutPage } from './layout/AboutPageLayout.js';
import { generationPage } from './layout/GenerationPageLayout.js';
import { promptEditingPage } from './layout/PromptEditingPageLayout.js';
import { controlsPage } from './layout/ControlsPageLayout.js';
import { modelSourcesPage } from './layout/ModelSourcesPageLayout.js';
import { libraryPage } from './layout/LibraryPageLayout.js';
import { comfyuiPage } from './layout/ComfyUIPageLayout.js';
import { appearancePage } from './layout/AppearancePageLayout.js';

export type {
  SettingsPageLayout,
  SettingsRowLayout,
  SettingsSectionLayout,
} from './SettingsLayoutRows.js';

/** The workspace's pages in the Python application's navigation order. */
export const SETTINGS_LAYOUT: readonly SettingsPageLayout[] = [
  aboutPage,
  generationPage,
  promptEditingPage,
  controlsPage,
  modelSourcesPage,
  libraryPage,
  comfyuiPage,
  appearancePage,
];
