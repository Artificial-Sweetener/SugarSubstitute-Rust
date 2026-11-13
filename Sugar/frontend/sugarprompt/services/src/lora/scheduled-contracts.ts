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
/** Effective model data is prepared from parsed source and compatible host authority, independently of editor presentation. */
import type { LoraCatalogItem, LoraModelIdentity } from './contracts.js';

/** Hosts have already resolved upstream and field models; their order and metadata are authoritative within this context revision. */
export interface LoraSchedulingContextInput {
  readonly revision: string;
  readonly catalogRevision: string;
  readonly mode: 'library' | 'workflow';
  /** A host declaration of proven scheduling support for this field, rather than evidence inferred from LoRA syntax. */
  readonly supported: boolean;
  readonly upstream: readonly LoraCatalogItem[];
  readonly fields: readonly LoraCatalogItem[];
}

/** Captured, validated host context; library contexts contain no workflow model groups. */
export type PreparedLoraSchedulingContext = LoraSchedulingContextInput;

/** First effective occurrence owns identity, display and provider-authored words; weights do not decide trigger eligibility. */
export interface ScheduledLora {
  readonly model: LoraModelIdentity;
  readonly display: string;
  readonly triggerWords: readonly string[];
  readonly source: 'inline' | 'upstream' | 'field';
  /** Explicit insertion preserves complete configured entries; completion splits them separately. */
  readonly insertionText: string;
}

/** One literal completion row retains the effective model that supplied its configured word. */
export interface LoraTriggerSuggestion {
  readonly word: string;
  readonly display: string;
  readonly backendValue: string;
}

/** Prepared prefix lookup returns every first-matching replacement in effective order, with bounded immutable result reuse. */
export interface LoraTriggerIndex {
  search(prefix: string): readonly LoraTriggerSuggestion[];
}

/** Missing authority and incompatible revisions never become an empty successful schedule. */
export type EffectiveLoraSnapshot =
  | {
      readonly status: 'ready';
      readonly context: PreparedLoraSchedulingContext;
      readonly models: readonly ScheduledLora[];
      readonly triggers: LoraTriggerIndex;
    }
  | {
      readonly status: 'unavailable';
      readonly reason:
        | 'missing-context'
        | 'unsupported-scheduling'
        | 'stale-catalog'
        | 'resources-unavailable';
    };

/** Canonical effective models support action eligibility without building a completion prefix index. */
export type EffectiveLoraModelSnapshot =
  | Omit<Extract<EffectiveLoraSnapshot, { status: 'ready' }>, 'triggers'>
  | Extract<EffectiveLoraSnapshot, { status: 'unavailable' }>;
