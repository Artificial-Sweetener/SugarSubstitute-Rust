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
/** Supported framework-independent feature and host preference contracts. */
export {
  prepareSceneQueue,
  SceneQueueSession,
  validatePreparedSceneQueue,
} from './scenes/queue.js';
export type {
  SceneQueueAdapter,
  SceneQueueRequest,
  SceneQueueResult,
  SceneQueueOutcome,
  PreparedSceneQueue,
} from './scenes/queue.js';
export { analyzeWorkflowScenes } from './scenes/workflow.js';
export type {
  WorkflowScenePrompt,
  WorkflowSceneInput,
  WorkflowScene,
  WorkflowSceneDiagnostics,
  WorkflowSceneGeneration,
  WorkflowSceneAnalysis,
} from './scenes/contracts.js';
export { prepareWildcardCatalog } from './wildcards/catalog.js';
export type {
  WildcardCatalogInput,
  WildcardCsvSource,
  WildcardReference,
  WildcardResolution,
  WildcardSearchResult,
  WildcardSuggestion,
  WildcardCatalog,
} from './wildcards/contracts.js';
export { prepareTagCompletionCatalog } from './completion/catalog.js';
export { prepareSceneTitleCatalog } from './completion/scene-catalog.js';
export { resolveCompletionQuery } from './completion/query-routing.js';
export type { CompletionQueryIndexes, RoutedCompletionQuery } from './completion/query-routing.js';
export type { SceneTitleCatalogInput, SceneTitleCatalog } from './completion/scene-catalog.js';
export { findTagCompletions, findPromptTextCompletions } from './completion/result.js';
export { prepareLoraSchedulingContext } from './lora/scheduled-context.js';
export { schedulingFeatureContext } from './lora/scheduling-profile.js';
export { EffectiveLoraSession } from './lora/scheduled-session.js';
export type {
  LoraSchedulingContextInput,
  PreparedLoraSchedulingContext,
  EffectiveLoraSnapshot,
  ScheduledLora,
  LoraTriggerIndex,
  LoraTriggerSuggestion,
} from './lora/scheduled-contracts.js';
export type {
  BundledTag,
  TagCatalogInput,
  TagSuggestion,
  TagCompletionCatalog,
  TagCompletionResult,
  PromptTextSuggestion,
  PromptTextCompletionResult,
} from './completion/contracts.js';
export type {
  FeatureId,
  FeatureContext,
  FeatureDecision,
  FeaturePreferences,
  FeatureProfile,
  FeatureSnapshot,
  PreferenceAdapter,
  PreferencePatch,
  PreferenceOutcome,
} from './features/contracts.js';
export { featureDefinitions } from './features/registry.js';
export {
  resolveFeatureProfile,
  validateFeatureContext,
  retainFeatureProfile,
  captureFeatureContext,
  sameFeatureContext,
  featureEnabled,
} from './features/profile.js';
export { PreferenceSession } from './features/session.js';
export { defaultPreferences } from './features/preferences.js';
export type {
  EducationClaim,
  EducationOutcome,
  EducationSnapshot,
  NestedParenthesisTip,
  ParenthesisEducationAdapter,
} from './education/contracts.js';
export { ParenthesisEducationSession } from './education/session.js';
export { LoraResourceSession } from './lora/session.js';
export { prepareSceneLoraScopes } from './lora/scene-scopes.js';
export type { PreparedSceneLoraScopes } from './lora/scene-scopes.js';
export { SceneLoraSession } from './lora/scene-schedules.js';
export type { SceneLoraSchedule, PreparedSceneLoraSchedules } from './lora/scene-schedules.js';
export { isCatalogStableLoraResource } from './lora/resource-validity.js';
export { LoraPickerSession } from './lora/picker.js';
export { LoraPageSession, prepareLoraPage } from './lora/model-page.js';
export { prepareLoraMetadata, validatePreparedLoraMetadata } from './lora/metadata-capability.js';
export { prepareWildcardDiagnostic } from './wildcards/diagnostics.js';
export type { WildcardDiagnostic, WildcardDiagnosticPayload } from './wildcards/diagnostics.js';
export type { PreparedLoraMetadata } from './lora/metadata-capability.js';
export { LoraMetadataSession, prepareLoraMetadataMenu } from './lora/metadata-actions.js';
export type {
  LoraOutputImage,
  LoraMetadataRequest,
  LoraMetadataResult,
  LoraMetadataAdapter,
  LoraMetadataOutcome,
  LoraMetadataAcceptance,
  LoraMetadataMenuItem,
} from './lora/metadata-actions-contracts.js';
export type {
  LoraPageAdapter,
  LoraPageRequest,
  LoraPageResult,
  LoraPageOutcome,
} from './lora/model-page.js';
export type { LoraPickerAdapter, LoraPickerResult, LoraPickerOutcome } from './lora/picker.js';
export { createLoraCatalogAdapter } from './lora/catalog-adapter.js';
export type {
  LoraCompletionOptions,
  LoraCompletionCatalog,
  LoraSuggestion,
  LoraMatchKind,
} from './lora/contracts.js';
export type {
  LoraThumbnail,
  LoraModelIdentity,
  LoraCatalogItem,
  LoraCatalogSnapshot,
  LoraResource,
  LoraResourceAdapter,
  LoraResourceSnapshot,
  LoraWarmupOutcome,
} from './lora/contracts.js';
export { unsupportedSceneDiagnostics } from './document-diagnostics.js';
export type { UnsupportedSceneDiagnostic } from './document-diagnostics.js';
export { prepareDuplicateDiagnostics } from './diagnostics/duplicates.js';
export type {
  PreparedDuplicateGroup,
  DuplicateDiagnostic,
  PreparedDuplicateDiagnostics,
} from './diagnostics/duplicates.js';
export { captureConditioningContext } from './regions/context.js';
export type { ConditioningContext, ConditioningContextInput } from './regions/context.js';
export { resolveSharedRegionNames } from './regions/names.js';
export { ClipboardSession } from './clipboard/session.js';
export type {
  ClipboardGateway,
  ClipboardTransfer,
  ClipboardReadResult,
  ClipboardReadOutcome,
  ClipboardWriteResult,
  ClipboardWriteOutcome,
} from './clipboard/contracts.js';
export type { BooruLookupKey, BooruPostLookup } from './booru/links.js';
export { SpellcheckSession } from './diagnostics/spelling-session.js';
export type {
  SpellcheckGateway,
  SpellingAvailability,
  SpellingLookup,
  SpellingUnavailableCode,
} from './diagnostics/spelling-contracts.js';

export type {
  SearchContext,
  SearchRequest,
  SearchMatch,
  SearchResults,
  SearchHostIntent,
  SearchRevealAdapter,
  SearchNavigation,
  SearchOutcome,
  SearchSnapshot,
} from './search/contracts.js';

export { prepareWildcardGeneration } from './wildcards/generation.js';
export {
  ManagedWildcardLibrary,
  ManagedWildcardCommittedError,
} from './managed-wildcards/library.js';
export type {
  ManagedWildcardFile,
  ManagedWildcardSnapshot,
  ManagedWildcardStorageFailure,
  ManagedWildcardListResult,
  ManagedWildcardReadResult,
  ManagedWildcardChangeResult,
  ManagedWildcardStorage,
  ManagedWildcardFolderAdapter,
  ManagedWildcardFailure,
  ManagedWildcardRefreshOutcome,
  ManagedWildcardReadOutcome,
  ManagedWildcardChangeOutcome,
  ManagedWildcardFolderOutcome,
} from './managed-wildcards/contracts.js';
export type {
  WildcardCandidateSource,
  WildcardGenerationInput,
  WildcardReplacementDetail,
  WildcardGenerationIssue,
  WildcardGenerationResolution,
  WildcardGenerationOutcome,
  WildcardGenerationPass,
  WildcardGenerationPreparation,
} from './wildcards/generation-contracts.js';

export { AutocompleteListSession } from './autocomplete-lists/session.js';
export type {
  AutocompleteList,
  AutocompleteListSnapshot,
  AutocompleteListCommand,
  AutocompleteListHostResult,
  AutocompleteListStorage,
  PreparedAutocompleteLists,
  AutocompleteListOutcome,
  AutocompleteListSettlement,
  AutocompleteListOptions,
} from './autocomplete-lists/contracts.js';

export { AutocompleteListCommittedError } from './autocomplete-lists/committed.js';

export type { AutocompleteListCommitReceipt } from './autocomplete-lists/committed.js';
export type {
  SavedSegment,
  SavedSegmentScope,
  SavedSegmentContext,
  SavedSegmentAdapter,
  SavedSegmentRead,
  SavedSegmentCatalog,
} from './saved-segments/contracts.js';
export { SavedSegmentSession } from './saved-segments/session.js';
export {
  captureSavedSegmentContext,
  prepareSavedSegmentCatalog,
} from './saved-segments/preparation.js';
