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
/** Atomic host intent uses exact-source UTF-16 coordinates and prepared native configuration. */
import type { TagLexicon, PromptDocumentKind } from '../../core/src/index.js';
import type {
  ConditioningContextInput,
  FeatureProfile,
  TagCompletionCatalog,
  LoraCompletionCatalog,
  WildcardCatalog,
  SceneTitleCatalog,
} from '../../services/src/index.js';
import type { BooruPostLookup } from '../../services/src/booru/links.js';
import type { SpellcheckSession } from '../../services/src/diagnostics/spelling-session.js';
import type { SourceLineChromeOptions } from './source-line-contracts.js';
import type { LoraPageHost, LoraMetadataHost } from './lora-action-hosts.js';
import type { LoraSchedulingHost } from './lora-schedule.js';
import type { WheelMode } from './wheel-mode.js';
import type { SceneQueueHost } from './scene-queue.js';
import type { SavedSegmentHost } from './saved-segment-menu.js';
import type { FieldActionUtilities } from './field-action-utilities.js';
import type { DocumentCheckpoint, DocumentCheckpointRejection } from './document-checkpoint.js';

/** One native configuration contract serves mounting and atomic updates; prepared service output remains immutable. */
export interface HostConfiguration {
  /** Prepared field-menu picker intent delegates to the host's existing finite picker owner. */
  readonly fieldActionUtilities?: FieldActionUtilities | null;
  /** Finite preset storage and host-resolved model associations. */
  readonly savedSegmentHost?: SavedSegmentHost | null;
  readonly documentKind?: PromptDocumentKind;
  readonly conditioningContext?: ConditioningContextInput | null;
  /** The host theme accent as `#rrggbb`, from which regional rails rotate; null keeps the pinned base. */
  readonly regionAccent?: string | null;
  readonly readOnly?: boolean;
  readonly placeholder?: string;
  readonly richRendering?: boolean;
  readonly sourceLineChrome?: SourceLineChromeOptions;
  readonly featureProfile?: FeatureProfile;
  readonly wheelMode?: WheelMode;
  readonly tagLexicon?: TagLexicon | null;
  readonly tagCompletionCatalog?: TagCompletionCatalog | null;
  readonly loraCompletionCatalog?: LoraCompletionCatalog | null;
  readonly wildcardCatalog?: WildcardCatalog | null;
  readonly sceneTitleCatalog?: SceneTitleCatalog | null;
  /** Supported post/image-link lookup runs only through the finite native paste owner. Null retains literal clipboard text. */
  readonly booruPostLookup?: BooruPostLookup | null;
  /** Hosts retain dictionary/session ignores across mounts; native editor teardown releases its subscriptions only. */
  readonly spellcheckSession?: SpellcheckSession | null;
  readonly sceneQueueHost?: SceneQueueHost | null;
  readonly loraPageHost?: LoraPageHost | null;
  readonly loraMetadataHost?: LoraMetadataHost | null;
  readonly loraSchedulingHost?: LoraSchedulingHost;
}

/** Directional boundaries refer to final exact-source UTF-16, rejecting CRLF interiors. */
export interface HostSelection {
  readonly anchor: number;
  readonly head: number;
}

/** One native commit carries final meaning, permission, presentation and optional caret/focus intent. */
export type HostUpdate = {
  readonly configuration?: HostConfiguration;
  readonly selection?: HostSelection;
  readonly focus?: 'preserve' | 'editor';
} & (
  | { readonly mode: 'preserve' }
  | { readonly mode: 'exact-edit'; readonly source: string }
  | { readonly mode: 'baseline'; readonly source: string }
  | {
      readonly mode: 'restore-document';
      readonly checkpoint: DocumentCheckpoint;
    }
);

/** Expected admission failures never retain source-bearing work. */
export type HostUpdateOutcome =
  | {
      readonly status: 'applied' | 'unchanged';
      /** Requested native focus readiness belongs to the same admitted bundle. */ readonly focusReady?: Promise<boolean>;
    }
  | {
      readonly status: 'rejected';
      readonly reason: 'composing' | 'read-only' | 'superseded' | DocumentCheckpointRejection;
    };
