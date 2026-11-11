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
/** Supported pure semantics; range DTOs use exact-source UTF-16, while diagnostic visibility compares positions in one declared UTF-16 space. */
export type { SourceRange } from './offsets.js';
export type { PreparedDocumentValues, PreparedDocumentContainer } from './document/contracts.js';
export { prepareDocumentPromptScopes, admitDocumentPromptScope } from './document/prompt-scopes.js';
export type {
  PreparedDocumentPromptScope,
  PreparedDocumentPromptScopes,
  DocumentPromptScopeOutcome,
} from './document/prompt-scopes.js';
export { decodedRangeToSource } from './document/value-mapping.js';
export type { PreparedValueMapping } from './document/value-mapping.js';
export { prepareDocumentReplacement } from './document/replacement.js';
export { prepareWildcardCsvEncoding } from './document/csv.js';
export type { PreparedWildcardCsvEncoding, WildcardCsvCell } from './document/csv.js';
export type {
  DocumentReplacementResult,
  DocumentReplacementRejection,
} from './document/replacement.js';
export { unicodeLowerCase } from './prompt/lower.js';
export {
  prepareWildcardColumnAliases,
  type WildcardColumnAlias,
} from './wildcard/column-aliases.js';
export {
  prepareWildcardGroups,
  isNumericWildcardTag,
  wildcardBodyRange,
} from './wildcard/groups.js';
export type { WildcardGroup } from './wildcard/groups.js';
export { mutateWildcardTag } from './wildcard/tag-mutations.js';
export type { WildcardTagChange, WildcardTagMutation } from './wildcard/tag-mutations.js';
export { prepareTriggerInsertionSlot, triggerInsertionText } from './lora/trigger-insertion.js';
export type { TriggerInsertionTarget, TriggerInsertionSlot } from './lora/trigger-insertion.js';
export { parsePrompt, serializePrompt } from './prompt/parser.js';
export { parenthesisEscapes, mapVisibleSource } from './prompt/display.js';
export type { VisibleSource } from './prompt/display.js';
export { prepareTagQueries } from './completion/query.js';
export { prepareSceneQueries, sceneReplacement, isSceneTitle } from './completion/scene-query.js';
export type { SceneQuery, PreparedSceneQueries } from './completion/scene-query.js';
export { prepareLoraQueries } from './completion/lora-query.js';
export {
  prepareWildcardQueries,
  wildcardReplacement,
  isWildcardCompletionBody,
  wildcardGhostText,
} from './completion/wildcard-query.js';
export type { WildcardQuery, PreparedWildcardQueries } from './completion/wildcard-query.js';
export {
  loraScheduleWeight,
  loraScheduleText,
  loraReplacement,
  loraCompletionSuffix,
} from './completion/lora-text.js';
export type {
  LoraQuery,
  LoraReplacement,
  PreparedLoraQueries,
} from './completion/lora-contracts.js';
export {
  tagReplacementText,
  completionSuffix,
  suffixWithoutRightText,
  tagIsNoop,
  tagGhostText,
  tagReplacement,
} from './completion/text.js';
export type { TagQuery, PreparedTagQueries, TagReplacement } from './completion/contracts.js';
export { parseScenes, normalizeSceneTitle } from './prompt/scenes.js';
export { prepareDocumentSemantics, validateDocumentKind } from './document/semantics.js';
export type { PromptDocumentKind, PreparedDocumentSemantics } from './document/contracts.js';
export { materializeScenePrompt, prepareSceneContexts } from './scenes/context.js';
export type { ScenePositionContext, PreparedSceneContexts } from './scenes/context.js';
export { caseFold } from './prompt/text.js';
export { prepareDuplicateSegments } from './diagnostics/duplicate-segments.js';
export type {
  DuplicateOwnership,
  DuplicateOccurrence,
  DuplicateFinding,
  PreparedDuplicateSegments,
} from './diagnostics/duplicate-segments.js';
export { prepareDuplicateEdit } from './diagnostics/duplicate-edits.js';
export type { DuplicateEditOutcome } from './diagnostics/duplicate-edits.js';
export { isDiagnosticCommitBoundary, diagnosticIsActive } from './diagnostics/visibility.js';
export { normalizeWeights } from './normalization/weights.js';
export { implicitWeight } from './normalization/decimals.js';
export { prepareTagLexicon, normalizeTagIdentity } from './normalization/lexicon.js';
export { canonicalizeParentheses, isExplicitEmphasis } from './normalization/parentheses.js';
export { normalizeForStorage } from './normalization/storage.js';
export { normalizePasteRange, normalizeTypedEdit } from './normalization/local.js';
export { normalizeTypedDeletion } from './normalization/regions.js';
export type { ParenthesisOptions } from './normalization/parentheses.js';
export type {
  SourceNormalization,
  ParenthesisTransition,
  GeneratedEmphasis,
  TagLexicon,
  TypedSourceEdit,
} from './normalization/contracts.js';
export type {
  PromptDocument,
  PromptStructure,
  PromptSegment,
  EmphasisSpan,
  LoraSpan,
  WildcardSpan,
  SyntaxSpan,
  RegionStructure,
  RegionSeparator,
  RegionPartition,
  SceneDocument,
  SceneBlock,
  SceneMarker,
  WildcardSyntax,
} from './prompt/contracts.js';
export { prepareSceneHeadings } from './scenes/headings.js';
export { regionNameReplacement, validRegionName } from './prompt/region-naming.js';
export type { SceneHeading } from './scenes/headings.js';
export {
  changedRegionNames,
  changedRegionIdentities,
  regionNameIdentities,
  firstRegionNames,
  applyRegionNames,
} from './prompt/region-name-identity.js';
export type { RegionName, RegionNameApplication } from './prompt/region-name-identity.js';
export {
  REGION_PALETTE_BASE,
  regionHue,
  regionPaletteColor,
  parseRegionHex,
  formatRegionHex,
} from './prompt/region-palette.js';
export type { RegionRgb } from './prompt/region-palette.js';
export type { SourceReplacement } from './normalization/contracts.js';
export { ConditioningTopology } from './prompt/conditioning.js';
export type { ConditioningMode, ConditioningScope } from './prompt/conditioning.js';
export { prepareValueReorder, prepareDocumentValueReorders } from './reorder/derivation.js';
export {
  valueReorderLayout,
  valueReorderVacancy,
  valueReorderAdjacentTarget,
  emptyValueReorderPartitions,
} from './reorder/layout.js';
export { hideReorderChip, dropReorderChip, moveReorderChip } from './reorder/movement.js';
export {
  serializeValueReorder,
  serializeReorderPreview,
  reorderChipText,
} from './reorder/serialization.js';
export { captureReorderSelection, mapReorderSelection } from './reorder/selection.js';
export { reorderOriginBoundary } from './reorder/source-pieces.js';
export type {
  PreparedValueReorder,
  ReorderChip,
  ReorderState,
  ReorderLayout,
  ReorderRow,
  ReorderGap,
  ReorderDropTarget,
  EmptyReorderPartition,
  ReorderOutput,
  ReorderSelection,
  ReorderEnvelope,
  ReorderEnvelopeOccurrence,
  ReorderText,
  ReorderTextPiece,
  ReorderDerivedReason,
  ReorderOriginPiece,
  ReorderOriginComponent,
  ReorderOriginBoundary,
} from './reorder/contracts.js';
