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
/** Hosts supply LoRA identity, metadata and acquired thumbnail references; services never discover files or fetch images. */

/** Ready references remain host-owned; unavailability is explicit rather than an invented placeholder image. */
export type LoraThumbnail =
  | {
      readonly status: 'ready';
      readonly url: string;
      readonly width: number;
      readonly height: number;
    }
  | { readonly status: 'unavailable' };

/** Canonical host model identity is distinct from the authored source name; basename is the extensionless file label. */
export interface LoraModelIdentity {
  readonly promptName: string;
  readonly backendValue: string;
  readonly basename: string;
}

/** One prepared installed model carries literal metadata and host-acquired media without discovery or acquisition. */
export interface LoraCatalogItem {
  readonly model: LoraModelIdentity;
  readonly title: string;
  readonly subtitle: string | null;
  readonly triggerWords: readonly string[];
  readonly thumbnail: LoraThumbnail;
  /** A validated HTTP(S) model page or explicit absence; opening it belongs to a host action. */
  readonly modelPageUrl: string | null;
}

/** Host authority and canonical items form one immutable catalog revision; replace the adapter to publish changes. */
export interface LoraCatalogSnapshot {
  readonly revision: string;
  readonly authoritative: boolean;
  readonly items: readonly LoraCatalogItem[];
}

/** Exact requested names and canonical model metadata remain distinct; resolution never rewrites source. */
export type LoraResource =
  | (LoraCatalogItem & {
      readonly name: string;
      readonly resolution: 'found';
    })
  | {
      readonly name: string;
      readonly resolution: 'ambiguous';
      /** Known collision size is a safe integer of at least two; null means the host cannot enumerate candidates. */
      readonly candidateCount: number | null;
    }
  | {
      readonly name: string;
      /** Pending authority and unavailability are retryable; missing is an authoritative catalog decision. */
      readonly resolution: 'missing' | 'pending-authority' | 'unavailable';
    };

/** Immutable catalog identity includes host/profile/content/media changes. Replace the adapter when its catalog changes.
 * The host acquires thumbnail URLs, keeps their resources alive for the component, and respects abort before side effects.
 * Resolve returns exactly one record per unique requested name, without rewriting names or returning unsolicited records.
 */
export interface LoraResourceAdapter {
  readonly revision: string;
  /** Optional prepared suggestions share this catalog revision; absence leaves name completion unavailable. */
  readonly completionCatalog?: LoraCompletionCatalog;
  resolve(names: readonly string[], signal: AbortSignal): Promise<readonly LoraResource[]>;
}

/** External preparation is independent of editable source/history; ready means validated descriptors, including unresolved states.
 * Idle/loading/failed snapshots may retain catalog-valid records for already resolved names while new source coverage is prepared.
 * Replacing the adapter retires every record from the previous catalog.
 */
export interface LoraResourceSnapshot {
  readonly status: 'idle' | 'loading' | 'ready' | 'unavailable' | 'failed' | 'disposed';
  readonly resources: readonly LoraResource[];
}

/** Source/profile/catalog supersession, composition and missing capability remain explicit public outcomes. */
export type LoraWarmupOutcome =
  | { readonly status: 'ready'; readonly resources: readonly LoraResource[] }
  | {
      readonly status: 'cancelled' | 'disposed' | 'composing' | 'disabled' | 'unavailable';
    };

/** Hosts supply explicit searchable metadata by exact backend identity, replacing the default title/subtitle/trigger-word text for those items. */
export interface LoraCompletionOptions {
  readonly searchMetadata: readonly {
    readonly backendValue: string;
    readonly text: string;
  }[];
}

/** Ranking labels preserve the reference's distinctions for exact, prefix, substring and metadata matches. */
export type LoraMatchKind =
  | 'empty'
  | 'exact'
  | 'basename_prefix'
  | 'display_prefix'
  | 'path_prefix'
  | 'basename_substring'
  | 'display_substring'
  | 'path_substring'
  | 'metadata';

/** One ranked immutable catalog item retains host media/metadata and its deliberate canonical scheduling identity. */
export interface LoraSuggestion {
  readonly item: LoraCatalogItem;
  /** Source-safe name resolving to this exact installed item; colliding canonical names require an explicit backend filename. */
  readonly scheduleName: string;
  readonly score: number;
  readonly matchKind: LoraMatchKind;
  readonly display: string;
}

/** Catalog preparation is outside native input; each query returns every ranked match and reuses bounded immutable results. */
export interface LoraCompletionCatalog {
  readonly revision: string;
  readonly authoritative: boolean;
  /** Match literal name-prefix text with prepared scalar substring postings; callers own source/caret freshness. */
  search(query: string): readonly LoraSuggestion[];
}
