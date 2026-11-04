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
/** Assemble the supported native editing state; the controller owns its view and public lifetime. */
import { searchMatchExtension } from './search-matches.js';
import { savedSegments } from './saved-segment-menu.js';
import { savedSegmentActions } from './saved-segment-actions.js';
import { EditorState, Prec, Text, Transaction, type Extension } from '@codemirror/state';
import { EditorView, keymap, type ViewUpdate } from '@codemirror/view';
import {
  cursorDocEnd,
  cursorDocStart,
  defaultKeymap,
  historyKeymap,
  selectDocEnd,
  selectDocStart,
  temporarilySetTabFocusMode,
} from '@codemirror/commands';
import { encodeSource } from '../../core/src/line-endings.js';
import type { TagLexicon } from '../../core/src/index.js';
import { changesExactSource, exactSourceExtension } from './exact-source.js';
import { editorFreshness } from './freshness.js';
import {
  semanticStructures,
  initializeNativeStructure,
  prepareNativeStructure,
} from './structure.js';
import { authorshipExtension } from './authorship.js';
import { typedNormalization } from './typed-normalization.js';
import { promptHistory } from './prompt-history.js';
import { richProjectionExtension, installProjection } from './rich-projection.js';
import { prepareProjection } from './projection-preparation.js';
import { promptWrapLayout } from './prompt-wrap-layout.js';
import { semanticRefresh } from './semantic-refresh.js';
import { promptMouseSelection } from './mouse-selection.js';
import { nativeCaretSide } from './native-caret-side.js';
import { weightPreview } from './weight-preview.js';
import { keyboardWeightInteractions } from './weight-interactions.js';
import { pointerWeightInteractions } from './weight-pointer.js';
import { wheelWeightInteractions } from './weight-wheel.js';
import { exactWeightInteractions } from './exact-weight.js';
import { promptCompletions } from './prompt-completion.js';
import { loraPresentation } from './lora-presentation.js';
import { nativeLoraSchedule } from './lora-schedule.js';
import { loraSchedulePreparation } from './lora-schedule-preparation.js';
import { loraMedia } from './lora-media-state.js';
import { loraMediaPreparation, loraMediaErrors } from './lora-media-preparation.js';
import { deleteGrapheme } from './input-policy.js';
import { nativeGraphemes } from './native-graphemes.js';
import { compositionHistory } from './composition-history.js';
import { horizontalNavigation, verticalNavigation } from './native-navigation.js';
import { nativeInputEvents } from './input-events.js';
import { editorTheme } from './editor-theme.js';
import { loraTheme } from './lora-theme.js';
import { loraActiveFeedback } from './lora-active.js';
import { contextMenu } from './context-menu-state.js';
import { loraMetadataActions } from './lora-metadata-actions.js';
import { nativeContextMenu } from './context-menu-extension.js';
import { nativeLayout } from './native-layout.js';
import { sourceLinesExtension } from './source-lines.js';
import type { SourceLineSnapshot } from './source-line-contracts.js';
import type { EditorLayoutMetrics } from './contracts.js';
import { sceneErrors } from './scene-errors.js';
import { sceneQueuePublication, sceneQueueTargets } from './scene-queue.js';
import { sceneQueuePreparation } from './scene-queue-preparation.js';
import { spellingDiagnostics } from './spelling-diagnostics.js';
import { spellingProvider, spellingLexicon, spellingErrors } from './spelling-provider.js';
import { duplicateDiagnostics } from './duplicate-diagnostics.js';
import { duplicateIgnores } from './duplicate-ignore.js';
import { documentDiagnostics } from './document-diagnostics.js';
import { regionDraft } from './region-draft.js';
import { regionNameInteractions } from './region-name.js';
import { regionHoverInteractions, regionHoverObserver } from './region-hover.js';
import { regionTheme } from './region-theme.js';
import { regionCaret } from './region-caret.js';
import { nativeConditioning } from './conditioning-state.js';
import { regionHost, regionNameObserver } from './region-host.js';
import { clipboardCommands, type ClipboardObserver } from './native-clipboard.js';
import { synchronizedRegionNames } from './region-host-transaction.js';
import { reorderInteractions } from './reorder-interactions.js';
import type { RegionNameChange } from './region-host-contracts.js';
import { documentSeedExtensions, type NativeDocumentSeed } from './document-state.js';

/** Private lifetime wiring; consumers obtain this through mountEditor rather than assembling extensions. */
export interface NativeStateOptions {
  readonly menuPresentation?: import('./menu-presentation.js').PromptMenuFactory;
  /** Authenticated document-only retention is initialized under this construction's current configuration. */
  readonly document?: NativeDocumentSeed;
  /** Native menu intent delegates to the controller's existing atomic configuration authority. */
  readonly changeRichRendering: (enabled: boolean) => void;
  readonly clipboardObserver?: ClipboardObserver;
  readonly label: string;
  readonly placeholder: string;
  readonly configuration: readonly Extension[];
  readonly view: () => EditorView;
  /** In-flight native event observers stop before resolving a disposed surface. */
  readonly isAlive: () => boolean;
  readonly lexicon: () => TagLexicon | undefined;
  readonly publish: () => void;
  readonly update: (update: ViewUpdate) => void;
  readonly onLayout: ((metrics: EditorLayoutMetrics) => void) | undefined;
  readonly onSourceLines: ((snapshot: SourceLineSnapshot) => void) | undefined;
  readonly revision: () => number;
  /** Host observation reports accepted authored names; incoming canonical synchronization remains silent. */
  readonly onRegionNameChange?: (change: RegionNameChange) => void;
  /** Host observation reports pointer hover of prompt separators; linked host hover remains silent. */
  readonly onRegionHover?: (ordinal: number | null) => void;
  /** Source-free spelling failures are observed at the injected host boundary. */
  readonly onSpellcheckError?: (cause: unknown) => void;
  /** Unexpected browser media failure is observed at the host boundary; expected unavailable images retain fallback rendering. */
  readonly onLoraMediaError?: (cause: unknown) => void;
}

/** Create exact baseline text, native history and all current supported editing/rendering extensions. */
export function createNativeState(source: string, options: NativeStateOptions): EditorState {
  const encoded = encodeSource(source);
  const state = EditorState.create({
    doc: options.document?.doc ?? Text.of(encoded.logical.split('\n')),
    ...(options.document ? { selection: options.document.selection } : {}),
    extensions: [
      ...(options.document ? documentSeedExtensions(options.document) : []),
      exactSourceExtension(source),
      editorFreshness,
      searchMatchExtension,
      semanticStructures,
      nativeConditioning,
      regionDraft,
      documentDiagnostics,
      duplicateIgnores,
      duplicateDiagnostics,
      spellingDiagnostics,
      spellingProvider,
      spellingLexicon.of(options.lexicon),
      ...(options.onSpellcheckError ? [spellingErrors.of(options.onSpellcheckError)] : []),
      sceneErrors,
      options.configuration,
      regionNameObserver.of({
        revision: options.revision,
        observe: options.onRegionNameChange,
      }),
      regionHost,
      ...(options.onRegionHover ? [regionHoverObserver.of(options.onRegionHover)] : []),
      authorshipExtension,
      compositionHistory,
      typedNormalization(options.lexicon),
      promptHistory,
      nativeGraphemes,
      clipboardCommands(options.lexicon, options.clipboardObserver),
      loraPresentation,
      nativeLoraSchedule,
      loraMedia,
      richProjectionExtension,
      promptWrapLayout,
      loraMetadataActions,
      sceneQueuePublication,
      sceneQueueTargets,
      sceneQueuePreparation,
      contextMenu,
      savedSegments,
      savedSegmentActions,
      nativeContextMenu(options.changeRichRendering, options.menuPresentation),
      semanticRefresh,
      loraSchedulePreparation,
      loraMediaPreparation,
      ...(options.onLoraMediaError ? [loraMediaErrors.of(options.onLoraMediaError)] : []),
      promptMouseSelection,
      nativeCaretSide,
      reorderInteractions,
      weightPreview,
      keyboardWeightInteractions,
      pointerWeightInteractions,
      wheelWeightInteractions,
      exactWeightInteractions,
      regionNameInteractions,
      regionHoverInteractions,
      regionCaret,
      promptCompletions,
      EditorState.transactionFilter.of((transaction) =>
        transaction.startState.readOnly &&
        changesExactSource(transaction) &&
        !transaction.annotation(synchronizedRegionNames)
          ? []
          : transaction,
      ),
      EditorView.contentAttributes.of({
        'aria-label': options.label,
        'aria-multiline': 'true',
        role: 'textbox',
        tabindex: '0',
        spellcheck: 'false',
        autocorrect: 'off',
        autocapitalize: 'off',
      }),
      EditorView.lineWrapping,
      Prec.high(
        keymap.of([
          { key: 'Backspace', run: (view) => deleteGrapheme(view, false) },
          { key: 'Delete', run: (view) => deleteGrapheme(view, true) },
          {
            key: 'ArrowLeft',
            run: (view) => horizontalNavigation(view, false, false),
            shift: (view) => horizontalNavigation(view, false, true),
            preventDefault: true,
          },
          {
            key: 'ArrowRight',
            run: (view) => horizontalNavigation(view, true, false),
            shift: (view) => horizontalNavigation(view, true, true),
            preventDefault: true,
          },
          {
            key: 'ArrowUp',
            run: (view) => verticalNavigation(view, false, false),
            shift: (view) => verticalNavigation(view, false, true),
            preventDefault: true,
          },
          {
            key: 'ArrowDown',
            run: (view) => verticalNavigation(view, true, false),
            shift: (view) => verticalNavigation(view, true, true),
            preventDefault: true,
          },
          { key: 'Home', run: cursorDocStart, shift: selectDocStart },
          { key: 'End', run: cursorDocEnd, shift: selectDocEnd },
          { key: 'Escape', run: temporarilySetTabFocusMode },
          {
            key: 'Tab',
            run: () => true,
            shift: () => true,
            preventDefault: true,
          },
          ...historyKeymap,
          ...defaultKeymap.filter(
            (binding) =>
              !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(binding.key ?? ''),
          ),
        ]),
      ),
      EditorView.updateListener.of(options.update),
      nativeInputEvents(options.view, options.lexicon, options.publish, options.isAlive),
      editorTheme,
      loraTheme,
      loraActiveFeedback,
      regionTheme,
      nativeLayout(options.onLayout, options.revision),
      sourceLinesExtension(options.revision, options.onSourceLines),
    ],
  });
  const initialized = initializeNativeStructure(state);
  return initialized.update({
    effects: installProjection.of(
      prepareProjection(initialized, prepareNativeStructure(initialized)),
    ),
    annotations: Transaction.addToHistory.of(false),
  }).state;
}
