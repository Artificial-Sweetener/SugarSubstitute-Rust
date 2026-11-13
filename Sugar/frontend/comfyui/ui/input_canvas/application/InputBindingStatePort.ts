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
/** Expose durable mask state without allowing native graph or browser APIs into binding policy. */
import type { InputPreviewSettings } from '../domain/InputLoader.js';
import type { InputBinding } from '../domain/InputBinding.js';
/** Supply exact owner state, authored source values and committed file-selection intent. */
export interface InputBindingStatePort {
  read(): InputBinding | null;
  load(): Promise<InputBinding | null>;
  write(state: InputBinding): void;
  files(): readonly string[];
  settings(): InputPreviewSettings;
  subscribe(changed: (intent: 'append' | 'replace' | null) => void): () => void;
}
