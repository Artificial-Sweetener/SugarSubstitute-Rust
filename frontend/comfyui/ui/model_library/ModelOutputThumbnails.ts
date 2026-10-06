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
/** Define completed output-image choices independently of Comfy and thumbnail transport. */
export interface ModelOutputThumbnailChoice {
  readonly id: string;
  readonly sceneKey: string;
  readonly sceneTitle: string;
  readonly sceneOrder: number;
  readonly sourceKey: string;
  readonly sourceLabel: string;
  readonly batch: number;
}

export interface ModelOutputThumbnails {
  choices(): readonly ModelOutputThumbnailChoice[];
  activeChoice(): ModelOutputThumbnailChoice | null;
  image(id: string, signal: AbortSignal): Promise<File>;
}
