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
import type { MaskEditSettings } from '../domain/MaskEditSettings.js';
import type { InputLoader, InputPreviewSettings } from '../domain/InputLoader.js';
import type {
  CanvasInputSource,
  CanvasSourceContext,
  CanvasPixelRaster,
} from '../../../../../integrations/contracts/CanvasProtocol.generated.js';
import type { InputPreviewInterpretation } from './InputPreviewInterpretation.js';
import type { MaskEditingPort } from './MaskEditingPort.js';
import type { RegionPresentation } from './WorkflowRegionalPrompts.js';

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

/** Answer per-canvas capability questions that change how sources are defaulted and edited. */
export interface InputCanvasCapabilities {
  /** Whether the canvas positions inserted layers, which mask-pair editing requires. */
  editable(canvasId: string): boolean;
  /** Whether the canvas provisions workflow surfaces, which then own loader source defaults. */
  provisions(canvasId: string): boolean;
}

/** Connect one loader card to the input surfaces a workflow-surface canvas holds for it. */
export interface InputSurfaceCardPort {
  /** Show the loader's surface on the Input route; entry names a region of an ordered mask. */
  activate(entry: string | null): void;
  /** Show an image loader's newly chosen file on the Input route once its surface exists. */
  openChosen(): void;
  /** Hover a region of an ordered mask loader in every linked view, or clear the hover. */
  hover(entry: string | null): void;
  /** The region entry another linked view currently shows as hovered, or null. */
  hovered(): string | null;
  /** Observe changes to the linked hover. */
  subscribeHover(listener: () => void): () => void;
  /** Name and colour a region of an ordered mask loader, or null for other entries. */
  region(entry: string): RegionPresentation | null;
  /** The loader's visual opacity in 0..1, or null while it has no provisioned masks. */
  opacity(): number | null;
  changeOpacity(opacity: number): void;
  /** Whether pictures can be imported as new regions of this ordered mask loader. */
  canImport(): boolean;
  /** Import pictures, stretched to the surface and read as gray levels, as new regions. */
  importMasks(files: readonly Blob[]): Promise<void>;
  subscribe(listener: () => void): () => void;
}

/** Describe authored source choices independently of the underlying canvas API. */
export interface InputEditorBinding {
  readonly loader: InputLoader;
  readonly state: InputBinding;
  readonly busy: boolean;
  readonly error: string | null;
  readonly preview: InputPreviewInterpretation;
  readonly maskEditing: MaskEditingPort;
  readonly canvasId: string;
  readonly canEditMask: boolean;
  /**
   * Whether the connected canvas provisions this loader's input surfaces. Its cards then present
   * the input canvas's own mask workflow instead of explicit source bindings.
   */
  readonly provisioned: boolean;
  readonly context: CanvasSourceContext | null;
  subscribe(listener: () => void): () => void;
  initialize(): Promise<void>;
  refreshSources(): Promise<void>;
  changeSource(source: CanvasInputSource): void;
  /**
   * Load a Comfy input file into the selected entry's provisioned mask. The surface provisioning
   * imports it when its size matches the mask's image, and reports a mismatch otherwise; either
   * way provisioning keeps owning the entry's source.
   */
  loadFile(file: string): void;
  select(id: string): void;
  add(): void;
  remove(): void;
  move(delta: number): void;
  changeMaskEdits(settings: MaskEditSettings): void;
  provision(sources: ReadonlyMap<string | null, CanvasInputSource>): void;
}

/** Query one graph-derived image relationship without exposing host nodes or reimplementing pairing policy. */
export interface InputImageAssociation {
  read(): InputEditorBinding | null;
  /** Admit image preparation only when this mask exclusively owns one scalar image source. */
  editable(): boolean;
  subscribe(listener: () => void): () => void;
}

/** Resolve graph-derived endpoint identities through the same source-state registry for any application consumer. */
export interface InputAssetBindingLookup {
  binding(endpoint: string): InputEditorBinding | null;
}
