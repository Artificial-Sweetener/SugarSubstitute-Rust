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
/** Expose loader-source state and interpretation without coupling views to native canvas implementation. */
import type { InputBinding } from '../domain/InputBinding.js';
import type { InputLoader, InputPreviewSettings } from '../domain/InputLoader.js';
import type {
  CanvasInputSource,
  CanvasSourceContext,
  CanvasPixelRaster,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type { InputPreviewInterpretation } from './InputPreviewInterpretation.js';

/** Read image source files and effective masks through their native codecs. */
export interface InputPreviewPort {
  interpret(
    source: CanvasPixelRaster,
    settings: InputPreviewSettings,
    maximumSize: number,
  ): Promise<CanvasPixelRaster>;
  load(
    file: string,
    settings: InputPreviewSettings,
    maximumSize: number,
  ): Promise<CanvasPixelRaster>;
}

/** Describe authored source choices independently of the underlying canvas API. */
export interface InputEditorBinding {
  readonly loader: InputLoader;
  readonly state: InputBinding;
  readonly busy: boolean;
  readonly error: string | null;
  readonly preview: InputPreviewInterpretation;
  readonly canvasId: string;
  readonly context: CanvasSourceContext | null;
  subscribe(listener: () => void): () => void;
  initialize(): Promise<void>;
  refreshSources(): Promise<void>;
  changeSource(source: CanvasInputSource): void;
  select(id: string): void;
  add(): void;
  remove(): void;
  move(delta: number): void;
}

/** Query one graph-derived image relationship without exposing host nodes or reimplementing pairing policy. */
export interface InputImageAssociation {
  read(): InputEditorBinding | null;
  subscribe(listener: () => void): () => void;
}

/** Resolve graph-derived endpoint identities through the same source-state registry for any application consumer. */
export interface InputAssetBindingLookup {
  binding(endpoint: string): InputEditorBinding | null;
}
