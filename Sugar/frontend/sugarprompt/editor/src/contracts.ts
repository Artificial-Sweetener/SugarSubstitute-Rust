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
/** Public native editing contracts; source offsets and document offsets are deliberately distinct. */
import type { FocusHandoff } from './focus-handoff-contracts.js';
import type {
  SearchContext,
  SearchRequest,
  SearchResults,
  SearchNavigation,
  SearchOutcome,
  SearchSnapshot,
  SearchRevealAdapter,
} from '../../services/src/search/contracts.js';
import type { FieldActionContext, FieldActionPreparation } from './field-actions-contracts.js';
import type { HostUpdate, HostUpdateOutcome } from './host-update.js';
import type { ClipboardAction, ClipboardCommandOutcome } from './native-clipboard.js';
import type {
  PromptStructure,
  WildcardSyntax,
  TagLexicon,
  ScenePositionContext,
  PromptDocumentKind,
  PreparedDocumentSemantics,
  ConditioningTopology,
  RegionName,
  PreparedDocumentPromptScopes,
} from '../../core/src/index.js';
import type {
  FeatureProfile,
  LoraResourceSnapshot,
  TagCompletionCatalog,
  LoraCompletionCatalog,
  WildcardCatalog,
  SceneTitleCatalog,
  ConditioningContextInput,
  ConditioningContext,
} from '../../services/src/index.js';
import type { WheelMode } from './wheel-mode.js';
import type {
  RegionNameInspection,
  RegionNameTarget,
  RegionNameSyncOutcome,
} from './region-host-contracts.js';
import type { LoraSchedulingHost } from './lora-schedule.js';
import type { LoraPageHost, LoraMetadataHost } from './lora-action-hosts.js';
import type { SceneQueueHost } from './scene-queue.js';
import type { SourceLineSnapshot, SourceLineChromeOptions } from './source-line-contracts.js';
import type {
  PreparedSourceEdit,
  SourceEditPreparation,
  SourceEditOutcome,
  SourceEditReservation,
  ReservedSourceEdit,
} from './prepared-edits.js';

/** Source inspection shares actual canonical scopes; syntax ranges are decoded scope-local UTF-16 and mappings expose exact-source UTF-16. */
interface ReadyStructureInspection {
  readonly status: 'ready';
  readonly scopes: PreparedDocumentPromptScopes;
  readonly documentSemantics: PreparedDocumentSemantics;
  /** Exact ordered LoRA names from admitted values and effective prompt scenes, for host warmup. */
  readonly loraResourceNames: readonly string[];
}

/** Composition defers inspection; prompt-only whole-document grammar is available after narrowing the ready kind. */
export type StructureInspection =
  | (ReadyStructureInspection & {
      readonly kind: 'prompt';
      /** Prompt syntax and scenes retain their exact-source UTF-16 public ranges. */
      readonly structure: PromptStructure;
    })
  | (ReadyStructureInspection & {
      readonly kind: 'wildcard-txt' | 'wildcard-csv';
    })
  | { readonly status: 'composing' };

/** Prepared conditioning is source-current, presentation-independent and explicitly unavailable during composition or candidate editing. */
export type ConditioningInspection =
  | {
      readonly status: 'ready';
      readonly context: ConditioningContext;
      readonly topology: ConditioningTopology;
    }
  | { readonly status: 'composing' }
  | {
      readonly status: 'unsupported';
      readonly documentKind: Exclude<PromptDocumentKind, 'prompt'>;
    };

/** Read-only source scene inspection is separate from host queue authority; all bounds use exact-source UTF-16. */
export type SceneContextInspection =
  | {
      readonly status: 'ready';
      readonly context: ScenePositionContext;
      readonly effectivePromptTexts: readonly string[];
    }
  | { readonly status: 'composing' }
  | {
      readonly status: 'unsupported';
      readonly documentKind: Exclude<PromptDocumentKind, 'prompt'>;
    };

/** Prepared native measurements in CSS-local pixels; scaleY maps pointer deltas from screen pixels. */
export interface EditorLayoutMetrics {
  readonly revision: number;
  readonly contentHeight: number;
  readonly lineHeight: number;
  readonly verticalPadding: number;
  readonly outerPadding: number;
  readonly viewportHeight: number;
  readonly viewportWidth: number;
  readonly scaleY: number;
}

/** Read-only snapshot derived from CodeMirror, including directional exact-source UTF-16 selection. */
export interface EditorSnapshot {
  readonly documentKind: PromptDocumentKind;
  /** Exact spelling from the native document; composing snapshots include transient preedit rather than committed submission text. */
  readonly source: string;
  readonly anchor: number;
  readonly head: number;
  readonly documentAnchor: number;
  readonly documentHead: number;
  readonly revision: number;
  /** Native composition has started, including the interval before its first preedit document mutation. */
  readonly composing: boolean;
  readonly focused: boolean;
  readonly readOnly: boolean;
  readonly undoDepth: number;
  readonly redoDepth: number;
  readonly richRendering: boolean;
  readonly rendering: 'raw' | 'pending' | 'ready' | 'composing';
  /** Source-current semantic readiness is independent of rich/raw presentation. */
  readonly semantics: 'pending' | 'ready' | 'composing';
}

/** Deliberate public component API. All source/selection arguments use exact-source UTF-16 boundaries. */
export interface PromptEditorApi {
  /** Observe mounted aggregate field contribution support, including read-only rendering. */
  fieldActionsAvailable(): boolean;
  /** Capture current native source/selection before host focus effects; host dismissal must release the contribution. */
  prepareFieldActions(context: FieldActionContext): Promise<FieldActionPreparation>;
  /** Retain a document-only native checkpoint after measured input settlement; disposal and cancellation cannot publish it. */
  captureDocument(
    signal?: AbortSignal,
  ): Promise<import('./document-checkpoint.js').DocumentCaptureOutcome>;
  /** Issue a query/field request against the current exact-source revision. */
  setSearchContext(context: SearchContext | null): SearchRequest | null;
  /** Publish exact-source UTF-16 match ranges; CRLF-interior boundaries reject explicitly. */
  publishSearchMatches(results: SearchResults): SearchOutcome;
  /** Host visibility then native selection, scroll and focus follow explicit intent. */
  navigateSearchMatch(intent: SearchNavigation): Promise<SearchOutcome>;
  /** Clear local matches and request host filter removal; optional focus preserves viewport. */
  clearSearch(focus?: boolean): Promise<SearchOutcome>;
  /** Observe source-free field and active-match identity. */
  searchSnapshot(): SearchSnapshot;
  /** Replace the finite host field-reveal boundary. */
  setSearchRevealAdapter(adapter: SearchRevealAdapter | null): void;
  /** Validate and prepare a complete host bundle before one native adoption; baseline defaults to final source end. */
  updateHost(update: HostUpdate): HostUpdateOutcome;
  /** Capture source-free intent before callbacks; reserved-source retains harmless selection changes while its native source capability still validates freshness. */
  focusHandoff(purpose?: 'panel' | 'reserved-source'): FocusHandoff;
  /** Copy/cut/paste through the native browser boundary; delayed work rejects changed source/selection, composition, permissions and disposal explicitly. */
  clipboard(action: ClipboardAction): Promise<ClipboardCommandOutcome>;
  /** Inspect trimmed ordinal names and capture a same-editor source/context handle; unavailable during composition or candidate editing. */
  inspectRegionNames(): RegionNameInspection;
  /** Apply canonical host names once, preserving exact endings, directional selection and mapped native history; host loading also updates read-only peers. */
  synchronizeRegionNames(
    names: readonly RegionName[],
    target: RegionNameTarget,
  ): RegionNameSyncOutcome;
  /** Mirror another view's hover on a prompt document's separator ordinal, or clear it with null; pointer hover is never overridden and linked hover is never reported. Returns whether the separator is shown hovered. Malformed ordinals throw TypeError. */
  setLinkedRegionHover(ordinal: number | null): boolean;
  /** Replace validated endpoint/topology authority without source/history edits; context changes retire captured work. */
  setConditioningContext(value: ConditioningContextInput | null): void;
  /** Inspect source-current exact-source UTF-16 scopes; unavailable host context retains independent ownership. */
  inspectConditioning(): ConditioningInspection;
  /** Replace document meaning without changing source, selection or history; malformed kinds throw. */
  setDocumentKind(kind: PromptDocumentKind): void;
  /** Read current source/selection; returned snapshots never own mutable editing state. */
  snapshot(): EditorSnapshot;
  /** Observe prepared visible source-line rectangles and the zero-based current line without measuring. */
  sourceLineSnapshot(): SourceLineSnapshot;
  /** Set native row backgrounds and extra CSS-local content inset without altering authoring history. */
  setSourceLineChrome(options: SourceLineChromeOptions): void;
  /** Choose presentation without source/history changes; true means changed, false means already selected. Malformed intent throws TypeError. */
  setRichRendering(enabled: boolean): boolean;
  /** Inspect cached canonical value scopes; narrow kind for prompt scenes and whole-prompt structure. Syntax ranges use decoded scope-local UTF-16 with exact-source mappings. */
  inspectStructure(profile?: WildcardSyntax): StructureInspection;
  /** Inspect the scene at an exact-source UTF-16 position or the native directional head, and every distinct materialized prompt. Invalid/CRLF-interior bounds reject. */
  inspectSceneContext(position?: number): SceneContextInspection;
  /** Publish normalized nonempty scene error keys for the current source revision. Stale revisions/composition return false; malformed keys/revisions throw. Edits retire host errors. */
  setSceneErrorKeys(keys: readonly string[], sourceRevision: number): boolean;
  /** Load an exact baseline and reset history; rejects during active composition. */
  loadBaseline(source: string): boolean;
  /** Replace exact-source UTF-16 bounds in one history event. A current continuation preserves its original restoration intent; stale continuations reject. */
  replaceSource(from: number, to: number, source: string, continuation?: FocusHandoff): boolean;
  /** Prepare a source-free exact edit handle against current source/context; selection and history remain unchanged. */
  prepareSourceEdit(from: number, to: number, source: string): SourceEditPreparation;
  /** Apply a same-editor prepared edit once. Optional same-editor current continuation retains its original focus intent; stale source/context/continuation, composition and read-only state reject explicitly. */
  applySourceEdit(command: PreparedSourceEdit, continuation?: FocusHandoff): SourceEditOutcome;
  /** Reserve an exact-source UTF-16 range for a later host result without changing source or selection. */
  reserveSourceEdit(from: number, to: number): SourceEditReservation;
  /** Consume a same-editor reservation once. Optional current continuation retains the original handoff; stale source/context/continuation never receives the replacement. */
  applyReservedSourceEdit(
    command: ReservedSourceEdit,
    source: string,
    continuation?: FocusHandoff,
  ): SourceEditOutcome;
  /** Insert ordinary text under rich normalization or raw exact editing in one history event. Optional current continuation retains its original handoff; stale continuation/read-only/composition rejects. */
  insertText(text: string, continuation?: FocusHandoff): boolean;
  /** Observe one immutable prepared host tag snapshot; changing it preserves source and history. */
  setTagLexicon(lexicon: TagLexicon | undefined): void;
  /** Observe a prepared immutable tag completion catalog; replacement dismisses source-bound options without changing source/history. */
  setTagCompletionCatalog(catalog: TagCompletionCatalog | null): void;
  /** Replace prepared model suggestions, retiring the active completion context without source/history mutation. */
  setLoraCompletionCatalog(catalog: LoraCompletionCatalog | null): void;
  /** Replace prepared wildcard metadata, retiring source-bound commands while preserving source and native history. */
  setWildcardCatalog(catalog: WildcardCatalog | null): void;
  /** Replace immutable host titles, retiring source-bound suggestions while preserving native source/history. */
  setSceneTitleCatalog(catalog: SceneTitleCatalog | null): void;
  /** Replace the external page capability and its finite outcome/error observers, retiring held actions without source/history changes. */
  setLoraPageHost(host: LoraPageHost | null): void;
  /** Publish queue eligibility for the current source revision; stale source or unfinished composition returns false. */
  setSceneQueueHost(host: SceneQueueHost | null): boolean;
  /** Replace canonical metadata/output capabilities without changing editable source/history. */
  setLoraMetadataHost(host: LoraMetadataHost | null): void;
  /** Replace prepared scheduling authority; incompatible publications retire source-bound suggestions and commands. */
  setLoraSchedulingHost(host: LoraSchedulingHost): void;
  /** Set directional selection; invalid boundaries and CRLF interiors throw RangeError. */
  selectSource(anchor: number, head?: number): void;
  /** Focus and scroll the native surface; await readiness before immediate scripted input. False means retired or focus moved. */
  focus(): Promise<boolean>;
  /** Undo through CodeMirror history; returns false when unavailable/read-only. */
  undo(): boolean;
  /** Redo through CodeMirror history; returns false when unavailable/read-only. */
  redo(): boolean;
  /** Enable or disable editing while retaining selection, copy and inspection; disabling discards unfinished preedit and preserves original source, selection and prior history. Denied native startup settles before renewed editing. Newer synchronous host permission requests take precedence during cancellation. */
  setReadOnly(value: boolean): void;
  /** Observe service-validated immutable LoRA descriptors for this source revision; stale revisions return false without publication. */
  setLoraResources(snapshot: LoraResourceSnapshot, sourceRevision: number): boolean;
  /** Observe one resolved profile supplied by the component/service owner; never change source or history. */
  setFeatureProfile(profile: FeatureProfile): void;
  /** Observe validated token-wheel authorization intent; replaces dwell/activation without source or history changes. */
  setWheelMode(mode: WheelMode): void;
  /** Remove the component's resources; subsequent calls reject. */
  destroy(): void;
}
