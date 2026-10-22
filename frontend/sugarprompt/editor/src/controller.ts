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
/** Mount one native CodeMirror authoring surface with exact-source and finite resource ownership. */
import { requireValue } from '../../core/src/required.js';
import { NativeSearch } from './native-search.js';
import { nativeMenuOwner } from './context-menu-extension.js';
import { NativeHostUpdates } from './native-host-updates.js';
import { NativeConfiguration } from './native-configuration.js';
import { type EditorState } from '@codemirror/state';
import { nativeSourceCommands } from './native-source-commands.js';
import type { EditorOptions } from './editor-options.js';
import { createNativeState } from './native-state.js';
import { NativeTooltipParent } from './tooltip-parent.js';
import { prepareLoraMetadataActions, installLoraMetadataActions } from './lora-metadata-actions.js';
import { nativeSourceLines } from './source-lines.js';
import type { EditorSnapshot, PromptEditorApi } from './contracts.js';
import { changesExactSource } from './exact-source.js';
import {
  installLoraPresentation,
  loraPresentation,
  prepareLoraPresentation,
} from './lora-presentation.js';
import { NativeSurface } from './native-surface.js';
import { editorFreshness } from './freshness.js';
import {
  prepareNativeSnapshot,
  sameNativeSnapshot,
  materializeNativeSnapshot,
  type PreparedNativeSnapshot,
} from './native-snapshot.js';
import {
  snapshotPublicationMode,
  type NativeSnapshotPublicationMode,
} from './native-snapshot-policy.js';
import { authoredImplicitDepth } from './normalization-events.js';
import { featureProfile } from './features.js';
import { featureEnabled } from '../../services/src/index.js';
import {
  inspectSceneContext,
  inspectStructure,
  inspectConditioning,
} from './semantic-inspection.js';
import { installSceneErrors, prepareSceneErrors } from './scene-errors.js';
import { regionHost } from './region-host.js';
import { nativeCompositionStarted } from './input-settlement.js';
import { NativeEditingPermission } from './native-editing-permission.js';
import type { NativeDocumentSeed } from './document-state.js';
/** Mount the public editor lifetime; all input routes through native transactions. */
export function mountEditor(parent: HTMLElement, options: EditorOptions): PromptEditorApi {
  let search: NativeSearch | undefined = undefined;
  let sourceCommands: ReturnType<typeof nativeSourceCommands> | undefined;
  let disposeSurface: (() => void) | undefined;
  const editingPermission = new NativeEditingPermission(options.readOnly);
  const configuration = new NativeConfiguration(options);
  let disposed = false,
    revision = 0;
  let delivered: PreparedNativeSnapshot | null = null;
  let deliveryIssue: object | null = null;
  /** Guard finite lifetime rather than permitting publication into a destroyed editor. */
  function alive(): void {
    if (disposed) throw new Error('Editor disposed');
  }
  /** Return a derived snapshot with both exact-source and logical-document UTF-16 coordinates. */
  function snapshot(): EditorSnapshot {
    alive();
    return materializeNativeSnapshot(prepareNativeSnapshot(surface.view, revision));
  }
  /** Preserve ordinary observations and suppress only equal certified presentation output, with exact reentrant delivery ownership. */
  function publish(mode: NativeSnapshotPublicationMode): void {
    if (disposed || hostUpdates?.publishing) return;
    const view = surface.view,
      state = view.state,
      publishedRevision = revision;
    search?.observe();
    if (disposed || surface.view !== view || view.state !== state) return;
    const prepared = prepareNativeSnapshot(view, publishedRevision);
    if (disposed || surface.view !== view || view.state !== state || revision !== publishedRevision)
      return;
    if (mode === 'current' || !delivered || !sameNativeSnapshot(delivered, prepared)) {
      const previous = delivered,
        previousIssue = deliveryIssue,
        issue = {};
      delivered = prepared;
      deliveryIssue = issue;
      try {
        options.onUpdate(materializeNativeSnapshot(prepared));
      } catch (cause) {
        if (deliveryIssue === issue) {
          delivered = previous;
          deliveryIssue = previousIssue;
        }
        throw cause;
      }
    }
    if (disposed || surface.view !== view || view.state !== state || revision !== publishedRevision)
      return;
    view.plugin(nativeSourceLines)?.publish();
  }
  const tooltipParent = new NativeTooltipParent(parent, options.label, options.tooltipTransport);
  /** Rebuild native configuration for a deliberate baseline restoration. */
  function createState(
    source: string,
    finalConfiguration = configuration.values,
    finalRevision = revision,
    finalReadOnly = surface?.view.state.readOnly ?? options.readOnly,
    document?: NativeDocumentSeed,
  ): EditorState {
    return createNativeState(source, {
      ...(document ? { document } : {}),
      /** Menu intent preserves native source/selection/history through the existing host adoption owner. */
      changeRichRendering: (enabled) => {
        if (disposed || nativeCompositionStarted(surface.view)) return;
        const outcome = updateHost({
          mode: 'preserve',
          configuration: { richRendering: enabled },
        });
        if (outcome.status === 'applied' && !disposed) options.onRichRenderingChange?.(enabled);
      },
      label: options.label,
      menuPresentation: options.menuPresentation,
      placeholder: finalConfiguration.placeholder,
      configuration: [
        ...configuration.extensions(finalConfiguration, finalRevision),
        tooltipParent.configuration,
        editingPermission.extension(finalReadOnly),
      ],
      view: () => surface.view,
      isAlive: () => !disposed,
      lexicon: () => configuration.values.tagLexicon,
      publish: () => publish('current'),
      onLayout: options.onLayout,
      onSourceLines: options.onSourceLines,
      ...(options.onLoraMediaError ? { onLoraMediaError: options.onLoraMediaError } : {}),
      ...(options.onSpellcheckError ? { onSpellcheckError: options.onSpellcheckError } : {}),
      revision: () => revision,
      ...(options.clipboardObserver ? { clipboardObserver: options.clipboardObserver } : {}),
      ...(options.onRegionNameChange ? { onRegionNameChange: options.onRegionNameChange } : {}),
      /** The view lifetime publishes snapshots and accepted-input education intent. */
      update: (update) => {
        if (update.transactions.some(changesExactSource)) revision++;
        if (
          update.docChanged ||
          update.selectionSet ||
          update.focusChanged ||
          update.transactions.length
        )
          publish(update.focusChanged ? 'current' : snapshotPublicationMode(update.transactions));
        if (
          !disposed &&
          surface.view.state === update.state &&
          featureEnabled(update.state.facet(featureProfile), 'emphasis')
        )
          for (const transaction of update.transactions) {
            const depth = transaction.annotation(authoredImplicitDepth);
            if (depth !== undefined) options.onNestedParentheses?.(depth);
          }
      },
    });
  }
  let surface: NativeSurface;
  const hostUpdates = new NativeHostUpdates({
    get surface() {
      return surface;
    },
    configuration,
    permission: editingPermission,
    createBaseline: createState,
    alive: () => !disposed,
    revision: () => revision,
    adoptBaseline: () => {
      revision++;
    },
    publish: () => publish('current'),
    ...(options.onHostConfigurationAccepted
      ? { accepted: options.onHostConfigurationAccepted }
      : {}),
  });
  const updateHost = hostUpdates.apply.bind(hostUpdates);
  /** Revoke controller authority before independently releasing each acquired collaborator in native ownership order. */
  function retire(failures: unknown[]): void {
    if (disposed) return;
    disposed = true;
    delivered = null;
    deliveryIssue = null;
    const commands = sourceCommands,
      observedSearch = search,
      native = disposeSurface;
    sourceCommands = undefined;
    search = undefined;
    disposeSurface = undefined;
    for (const release of [
      () => hostUpdates.dispose(),
      () => commands?.dispose(),
      () => observedSearch?.dispose(),
      () => native?.(),
      () => tooltipParent.dispose(),
    ]) {
      try {
        release();
      } catch (cause) {
        failures.push(cause);
      }
    }
  }
  /** Report raw single causes, including null/undefined, or ordered failures after all independent retirement attempts. */
  function report(failures: readonly unknown[]): void {
    if (failures.length === 1) throw failures[0];
    if (failures.length > 1) throw new AggregateError(failures, 'Native editor retirement failed');
  }
  try {
    surface = new NativeSurface(parent, createState(options.source), () => publish('current'));
    disposeSurface = () => surface.dispose();
    search = new NativeSearch(
      surface,
      () => revision,
      (request) => options.onSearchRefresh?.(request),
      (intent) => options.onSearchIntent?.(intent),
    );
    sourceCommands = nativeSourceCommands(
      () => surface.view,
      () => configuration.values.tagLexicon,
      alive,
      {
        validate: (continuation) => surface.currentHandoff(continuation),
        accept: (admission, continuation) => {
          if (admission === 'source') surface.admitSource(continuation);
          hostUpdates.supersede();
          if (admission === 'selection') surface.retireHandoff('preserve-input');
        },
      },
    );
    publish('current');
  } catch (cause) {
    const failures: unknown[] = [cause];
    retire(failures);
    report(failures);
    throw cause;
  }
  return {
    /** The installed plugin remains the sole field/local invocation owner. */
    fieldActionsAvailable() {
      return (
        !disposed &&
        !!surface.view.state.facet(nativeMenuOwner)?.(surface.view)?.fieldActionsAvailable()
      );
    },
    /** Dispose is an expected aggregate outcome; malformed anchor validation belongs to the native owner. */
    prepareFieldActions(context) {
      if (disposed) return Promise.resolve({ status: 'disposed' });
      const owner = surface.view.state.facet(nativeMenuOwner)?.(surface.view);
      if (!owner) throw new Error('Native menu owner unavailable');
      return owner.prepareFieldActions(context);
    },
    ...sourceCommands.api,
    updateHost,
    /** Capture a same-editor native document through the finite measured input owner. */
    captureDocument: (signal) => hostUpdates.captureDocument(signal),
    /** Host query identity starts one source-bound recomputation lifetime. */
    setSearchContext: (context) => {
      alive();
      return requireValue(search, 'controller').setContext(context);
    },
    /** Prepared host ranges preserve source and history. */
    publishSearchMatches: (results) => {
      alive();
      return requireValue(search, 'controller').publish(results);
    },
    /** Explicit host visibility completion precedes measured native navigation. */
    navigateSearchMatch: (intent) => {
      alive();
      return requireValue(search, 'controller').navigate(intent);
    },
    /** Clear removes native highlights and host filters with explicit focus intent. */
    clearSearch: (focus = false) => {
      alive();
      return requireValue(search, 'controller').clear(focus);
    },
    /** Observe current field/query/source/match identities. */
    searchSnapshot: () => {
      alive();
      return requireValue(search, 'controller').snapshot();
    },
    /** Replace the finite host visibility adapter. */
    setSearchRevealAdapter: (adapter) => {
      alive();
      requireValue(search, 'controller').setAdapter(adapter);
    },
    /** Capture the current mounted native handoff without retaining source. */
    focusHandoff: (purpose = 'panel') => {
      alive();
      if (purpose !== 'panel' && purpose !== 'reserved-source')
        throw new TypeError('Invalid focus handoff purpose');
      return surface.handoff(purpose);
    },
    /** Native publication owns names, freshness and minimal source synchronization. */
    inspectRegionNames() {
      alive();
      const owner = surface.view.plugin(regionHost);
      if (!owner) throw new Error('Regional host owner unavailable');
      return owner.inspect();
    },
    /** Canonical loading consumes one captured same-editor handle without creating an authored rename event. */
    synchronizeRegionNames(names, target) {
      alive();
      const owner = surface.view.plugin(regionHost);
      if (!owner) throw new Error('Regional host owner unavailable');
      return owner.synchronize(names, target);
    },
    /** Captured host classification retires source-bound work while the parser's actual document stays reusable. */
    setConditioningContext(value) {
      updateHost({
        mode: 'preserve',
        configuration: { conditioningContext: value },
      });
    },
    /** Explicit inspection delegates pure partition semantics to the current prepared native owner. */
    inspectConditioning() {
      alive();
      return inspectConditioning(surface.view);
    },
    /** Reconfiguration retires derived capability/commands while CodeMirror retains exact source and native history. */
    setDocumentKind(value) {
      updateHost({ mode: 'preserve', configuration: { documentKind: value } });
    },
    /** Admit the complete host through the configuration/service owner before stale eligibility can retire installed queue authority. */
    setSceneQueueHost(host) {
      configuration.prepare({ sceneQueueHost: host }, revision);
      alive();
      if (
        host &&
        (configuration.values.documentKind !== 'prompt' ||
          host.capability.sourceRevision !== revision ||
          nativeCompositionStarted(surface.view))
      ) {
        updateHost({
          mode: 'preserve',
          configuration: { sceneQueueHost: null },
        });
        return false;
      }
      updateHost({ mode: 'preserve', configuration: { sceneQueueHost: host } });
      return true;
    },
    /** Workflow titles change native command context without changing exact source, selection or history. */
    setSceneTitleCatalog(catalog) {
      updateHost({
        mode: 'preserve',
        configuration: { sceneTitleCatalog: catalog },
      });
    },
    /** External validation is source-bound; native selection, history and composition remain authoritative. */
    setSceneErrorKeys(keys, sourceRevision) {
      alive();
      if (!Number.isSafeInteger(sourceRevision) || sourceRevision < 0)
        throw new RangeError('Invalid scene source revision');
      if (
        sourceRevision !== revision ||
        configuration.values.documentKind !== 'prompt' ||
        nativeCompositionStarted(surface.view)
      )
        return false;
      const prepared = prepareSceneErrors(surface.view.state.field(editorFreshness).source, keys);
      surface.view.dispatch({ effects: installSceneErrors.of(prepared) });
      return true;
    },
    /** Replace prepared host choices and compile action trees outside native input. */
    setLoraMetadataHost(host) {
      updateHost({
        mode: 'preserve',
        configuration: { loraMetadataHost: host },
      });
    },
    /** Retire held source commands before installing a different compatible host publication. */
    setLoraSchedulingHost(host) {
      updateHost({
        mode: 'preserve',
        configuration: { loraSchedulingHost: host },
      });
    },
    /** Capability replacement leaves document/history intact and retires source-bound menu/request ownership. */
    setLoraPageHost(host) {
      updateHost({ mode: 'preserve', configuration: { loraPageHost: host } });
    },
    /** Publish one immutable host model catalog without changing source or native history. */
    setLoraCompletionCatalog(catalog) {
      updateHost({
        mode: 'preserve',
        configuration: { loraCompletionCatalog: catalog },
      });
    },
    /** Replace captured wildcard authority without modifying native source, selection or history. */
    setWildcardCatalog(catalog) {
      updateHost({
        mode: 'preserve',
        configuration: { wildcardCatalog: catalog },
      });
    },
    /** Replace immutable completion context without modifying CodeMirror's source or history. */
    setTagCompletionCatalog(catalog) {
      updateHost({
        mode: 'preserve',
        configuration: { tagCompletionCatalog: catalog },
      });
    },
    /** Read the prepared native owner; a missing extension remains an observable invariant failure. */
    sourceLineSnapshot() {
      alive();
      const sourceLines = surface.view.plugin(nativeSourceLines);
      if (!sourceLines) throw new Error('Native source-line owner unavailable');
      return sourceLines.snapshot();
    },
    /** Visual configuration uses supported reconfiguration, independently of source history. */
    setSourceLineChrome(value) {
      updateHost({
        mode: 'preserve',
        configuration: { sourceLineChrome: value },
      });
    },
    snapshot,
    /** Observe the service-owned preference without introducing native persistence or source history. */
    setWheelMode(value) {
      updateHost({ mode: 'preserve', configuration: { wheelMode: value } });
    },
    /** Change presentation through native configuration without creating a history event. */
    setRichRendering(enabled) {
      const outcome = updateHost({
        mode: 'preserve',
        configuration: { richRendering: enabled },
      });
      return outcome.status === 'applied';
    },
    /** Prepared host catalog identity remains immutable and free of input-time I/O. */
    setTagLexicon(lexicon) {
      updateHost({
        mode: 'preserve',
        configuration: { tagLexicon: lexicon ?? null },
      });
    },
    /** Prepare semantic data outside native typing/selection and keep unfinished IME source untouched. */
    inspectStructure(profile = { style: 'curly' }) {
      alive();
      return inspectStructure(surface.view, profile);
    },
    /** Scene text and source ownership reuse the semantic inspection owner. */
    inspectSceneContext(position) {
      alive();
      return inspectSceneContext(surface.view, position);
    },
    /** Baseline restoration replaces state deliberately and clears history. */
    loadBaseline(source) {
      return updateHost({ mode: 'baseline', source }).status !== 'rejected';
    },
    /** Delegate measured native focus to its finite readiness owner. */
    focus() {
      alive();
      return surface.focus();
    },
    /** Discard unfinished native input before revoking editability; a newer host permission or teardown wins. */
    setReadOnly(value) {
      updateHost({ mode: 'preserve', configuration: { readOnly: value } });
    },
    /** Index current validated service output outside input/paint; native source/history remain unchanged. */
    setLoraResources(snapshot, sourceRevision) {
      alive();
      if (!Number.isSafeInteger(sourceRevision) || sourceRevision < 0)
        throw new RangeError('Invalid LoRA source revision');
      if (sourceRevision !== revision) return false;
      const current = surface.view.state.field(loraPresentation);
      if (
        current.snapshot === snapshot &&
        current.sourceIdentity === surface.view.state.field(editorFreshness).source
      )
        return false;
      const prepared = prepareLoraPresentation(surface.view.state, snapshot);
      surface.view.dispatch({
        effects: [
          installLoraPresentation.of(prepared),
          installLoraMetadataActions.of(prepareLoraMetadataActions(surface.view.state, prepared)),
        ],
      });
      return true;
    },
    /** Reconfigure derived capabilities while preserving native authoring state and prepared semantics. */
    setFeatureProfile(profile) {
      updateHost({
        mode: 'preserve',
        configuration: { featureProfile: profile },
      });
    },
    /** Retire CodeMirror listeners once and reject future API calls, including teardown during native retirement. */
    destroy() {
      const failures: unknown[] = [];
      retire(failures);
      report(failures);
    },
  };
}
