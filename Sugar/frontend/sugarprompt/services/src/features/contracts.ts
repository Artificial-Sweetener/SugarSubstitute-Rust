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
/** Host-facing capability and preference contracts; no framework or editor state lives here. */
export type FeatureId =
  | 'emphasis'
  | 'danbooru_url_import'
  | 'wildcard_syntax'
  | 'wildcard_autocomplete'
  | 'autocomplete_ghost_text'
  | 'lora_syntax'
  | 'lora_autocomplete'
  | 'lora_picker'
  | 'lora_trigger_words'
  | 'segment_reorder'
  | 'spellcheck'
  | 'duplicate_segment_diagnostics';

/** Ordered registry entry used by settings and profile resolution. */
export interface FeatureDefinition {
  readonly id: FeatureId;
  readonly label: string;
  readonly aliases: readonly string[];
  readonly dependencies: readonly FeatureId[];
  readonly conflicts: readonly FeatureId[];
  readonly syntax: 'emphasis' | 'wildcard' | 'lora' | null;
  readonly legacyBase: boolean;
}

/** Authoritative user preferences; schema ownership and storage are supplied by the host. */
export interface FeaturePreferences {
  readonly allowed: Readonly<Record<FeatureId, boolean>>;
  readonly wheelMode: 'hover_dwell' | 'focus_required';
  readonly resolveOnGeneration: boolean;
}

/** Deliberate preference edits; generation and wheel preferences are independent of feature switches. */
export interface PreferencePatch {
  readonly allowed?: Readonly<Partial<Record<FeatureId, boolean>>>;
  readonly wheelMode?: FeaturePreferences['wheelMode'];
  readonly resolveOnGeneration?: boolean;
}

/** Validated host context; legacy field metadata retains its deliberate public compatibility boundary. */
export interface FeatureContext {
  readonly mode: 'library' | 'workflow';
  readonly field?: {
    readonly prompt_features?: readonly unknown[];
    readonly prompt_syntaxes?: readonly unknown[];
  };
  readonly loraScheduling?: boolean;
  readonly unavailableFeatures?: readonly FeatureId[];
}

/** A feature permission decision, distinct from whether its implementation has landed. */
export type FeatureDecision =
  | {
      readonly feature: FeatureId;
      readonly enabled: true;
      readonly disabledReason: null;
      readonly detail: null;
    }
  | {
      readonly feature: FeatureId;
      readonly enabled: false;
      readonly disabledReason: 'user_disabled' | 'field_disabled' | 'missing_service' | 'conflict';
      readonly detail: string | null;
    };

/** One immutable ordered profile consumed by every extension and settings projection. */
export interface FeatureProfile {
  readonly decisions: readonly FeatureDecision[];
  readonly rendererSyntax: readonly ('emphasis' | 'wildcard' | 'lora')[];
}

/** Expected host storage unavailability is data; unexpected failures reject the request. */
export type PreferenceRead =
  | { readonly status: 'ready'; readonly value: unknown }
  | { readonly status: 'unavailable' };

/** Persistence succeeds before an updated preference snapshot can be published. */
export type PreferenceWrite = { readonly status: 'saved' } | { readonly status: 'unavailable' };

/** Host storage boundary: decode/migrate your schema before read, preserve original bytes on failure.
 * Writes are serialized by the session. Abort must settle the request and prevent subsequent persistence.
 */
export interface PreferenceAdapter {
  /** Read decoded preferences; null denotes first use and selects defaults without an implicit write. */
  read(signal: AbortSignal): Promise<PreferenceRead>;
  /** Atomically persist the supplied validated snapshot, respecting disposal cancellation. */
  write(value: FeaturePreferences, signal: AbortSignal): Promise<PreferenceWrite>;
}

/** Finite preference operation outcomes never masquerade as successful persistence. */
export type PreferenceOutcome =
  | {
      readonly status: 'ready' | 'saved';
      readonly preferences: FeaturePreferences;
    }
  | { readonly status: 'unavailable' | 'invalid' | 'disposed' };

/** Public component observation; no source text or mutable service machinery is exposed. */
export interface FeatureSnapshot {
  readonly preferences: FeaturePreferences;
  readonly profile: FeatureProfile;
  readonly persistence: 'loading' | 'ready' | 'unavailable' | 'invalid' | 'failed';
}
