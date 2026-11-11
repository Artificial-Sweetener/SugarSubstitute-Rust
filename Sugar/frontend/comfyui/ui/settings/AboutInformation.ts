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
/** Define host-independent project identity and runtime version facts for unified settings. */
export interface AboutComponent {
  readonly name: string;
  readonly description: string;
  readonly author: string;
  readonly version: string;
  readonly url: string;
}

export interface AboutInformation {
  readonly components: readonly AboutComponent[];
  readonly warnings: readonly string[];
}

/** Supply runtime facts without coupling the page to HTTP or a desktop window implementation. */
export interface AboutInformationSource {
  readonly noticesUrl: string;
  load(): Promise<AboutInformation>;
  license(): Promise<string>;
}

/** Share project identity with browser and future native About presentations. */
export const sugarCubesProject = {
  name: 'SugarCubes',
  description: 'Composable workflow units for ComfyUI',
  author: 'Artificial Sweetener and contributors',
  url: 'https://github.com/Artificial-Sweetener/SugarCubes',
  summary:
    'Build, share and run reusable SugarCubes, compose workflows, manage models and work with images in an integrated ComfyUI workspace.',
} as const;
