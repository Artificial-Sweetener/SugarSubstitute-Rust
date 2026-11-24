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
/** The shape of the settings layout: each page, section and row's place and text. */
import type { SettingsIcon } from './SettingsModel.js';

/** A row's place and text. */
export interface SettingsRowLayout {
  readonly id: string;
  readonly title: string;
  readonly description?: string;
  readonly icon?: SettingsIcon;
  readonly keywords?: readonly string[];
  readonly wrap?: number;
}

/** A section's place and text. */
export interface SettingsSectionLayout {
  readonly id: string;
  readonly title: string;
  readonly subtitle?: string;
  readonly rows: readonly SettingsRowLayout[];
}

/** A page's place and text. */
export interface SettingsPageLayout {
  readonly id: string;
  readonly title: string;
  readonly subtitle: string;
  readonly icon: SettingsIcon;
  readonly searchable: boolean;
  readonly sections: readonly SettingsSectionLayout[];
}

/** A row's place and text, with only the fields it sets. */
export const row = (
  id: string,
  title: string,
  description: string,
  icon?: SettingsIcon,
  wrap?: number,
  keywords?: readonly string[],
): SettingsRowLayout => ({
  id,
  title,
  description,
  ...(icon ? { icon } : {}),
  ...(wrap ? { wrap } : {}),
  ...(keywords ? { keywords } : {}),
});

/** The workspace's pages in the Python application's navigation order. */
