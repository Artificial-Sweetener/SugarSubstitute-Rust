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
/** Native capability publications separate external ownership from source-derived menu state. */
import { Facet } from '@codemirror/state';
import type {
  LoraPageAdapter,
  LoraPageOutcome,
  PreparedLoraMetadata,
  LoraResourceAdapter,
  LoraMetadataOutcome,
} from '../../services/src/index.js';

/** One immutable page capability and source-free outcome/error observers. */
export interface LoraPageHost {
  readonly adapter: LoraPageAdapter;
  readonly observe: (outcome: LoraPageOutcome) => void;
  readonly report: (cause: unknown) => void;
}

/** A host action adopts a complete validated replacement through the existing resource owner after native freshness acceptance. */
export interface LoraMetadataHost {
  readonly capability: PreparedLoraMetadata;
  readonly catalogRevision: string;
  readonly accept: (adapter: LoraResourceAdapter) => void;
  readonly observe: (outcome: LoraMetadataOutcome) => void;
  readonly report: (cause: unknown) => void;
}

/** Deliberate configuration compartments own the single current page capability identity. */
export const loraPageHost = Facet.define<LoraPageHost | null, LoraPageHost | null>({
  combine: (values) => values[0] ?? null,
});

/** Choice/capability/catalog replacement retires metadata requests independently of source text. */
export const loraMetadataHost = Facet.define<LoraMetadataHost | null, LoraMetadataHost | null>({
  combine: (values) => values[0] ?? null,
});
