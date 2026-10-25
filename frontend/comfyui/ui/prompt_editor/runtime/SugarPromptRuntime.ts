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
/** Compose the adopted native editor for Cube fields while preserving its sole editing and drag owners. */
import { mountEditor } from '../../../../sugarprompt/editor/src/index.js';
import type {
  PromptEditorApi,
  EditorSnapshot,
  EditorLayoutMetrics,
} from '../../../../sugarprompt/editor/src/contracts.js';
import {
  LoraResourceSession,
  resolveFeatureProfile,
  defaultPreferences,
} from '../../../../sugarprompt/services/src/index.js';
import type {
  LoraResourceAdapter,
  LoraResourceSnapshot,
} from '../../../../sugarprompt/services/src/index.js';
import { SugarPromptMenu } from './SugarPromptMenu.js';
import { SugarPromptTooltip } from './SugarPromptTooltip.js';
import { installSugarPromptStyles } from './SugarPromptStyles.js';
import type { AppearanceSource } from '../../surface/appearance/AppearanceState.js';
import { applyOwnedTheme } from '../../surface/appearance/ThemeService.js';
import { markComfyPromptInput } from './ComfyPromptInput.js';
import { PromptMountReadiness } from '../PromptMountReadiness.js';
import { bundledPromptTagCatalog } from './BundledPromptTags.js';
import { createEditorField } from '../../surface/appearance/ControlFactory.js';
import type { EditorFieldControl } from '../../surface/appearance/EditorFieldControl.js';
import type { VisualStyle } from '../../surface/appearance/VisualStyle.js';
import { relocateControl } from '../../core/ControlPlacement.js';

/** Host callbacks carry committed source and measured geometry, never a second editable buffer. */
export interface SugarPromptMountOptions {
  readonly anchor: HTMLTextAreaElement;
  readonly source: string;
  readonly label: string;
  readonly readOnly: boolean;
  readonly commit: (source: string) => void;
  readonly layout: (height: number) => void;
  readonly report: (cause: unknown) => void;
  readonly appearance: AppearanceSource;
}

/** One field retains its native document across renderer presentations. */
export class SugarPromptEditor {
  readonly element: HTMLDivElement;
  readonly #api: PromptEditorApi;
  readonly #options: SugarPromptMountOptions;
  readonly #stopAppearance: () => void;
  readonly #content: HTMLDivElement;
  readonly #readiness: PromptMountReadiness;
  #field: EditorFieldControl | null = null;
  #fieldStyle: VisualStyle | null = null;
  #resources: LoraResourceSession | null = null;
  #adapter: LoraResourceAdapter | null = null;
  #snapshot: EditorSnapshot | null = null;
  #disposed = false;
  #scheduled = false;
  #resourceKey = '';
  #resourceGeneration = 0;
  #resourceSnapshot: LoraResourceSnapshot | null = null;
  #resourceRevision = -1;
  #inspectionRevision = -1;
  #publishing = false;
  #pendingRefresh: { source: string; readOnly: boolean } | null = null;
  #refreshQueued = false;
  #committedSource: string;
  #allocatedHeight: number | null = null;

  /** Mount all existing editing, rich projection, weight and Alt-reorder behavior through its public API. */
  constructor(document: Document, options: SugarPromptMountOptions) {
    installSugarPromptStyles(document);
    this.#options = options;
    this.#committedSource = options.source;
    this.element = document.createElement('div');
    this.element.className = 'sugarprompt-editor sc-cube-prompt-editor';
    this.element.dataset.scPromptEditor = '';
    this.element.dataset.scHostOpaque = '';
    // Comfy's ancestor capture handler requires this contract before descendants receive wheel input.
    this.element.dataset.captureWheel = 'true';
    this.element.style.width = '100%';
    this.element.style.minHeight = '128px';
    this.element.style.height = '128px';
    this.element.style.position = 'relative';
    this.#content = document.createElement('div');
    this.#readiness = new PromptMountReadiness();
    this.#content.style.cssText = 'width:100%;height:100%;min-width:0;min-height:0';
    options.anchor.before(this.element);
    this.#applyAppearance();
    this.#stopAppearance = options.appearance.subscribeAppearance(() => this.#applyAppearance());
    for (const type of ['keydown', 'keyup', 'pointerdown', 'contextmenu', 'wheel']) {
      this.element.addEventListener(type, (event) => event.stopPropagation());
    }
    const profile = resolveFeatureProfile(defaultPreferences, {
      mode: 'workflow',
      loraScheduling: false,
      unavailableFeatures: ['danbooru_url_import', 'wildcard_autocomplete', 'spellcheck'],
    });
    const tagCatalog = bundledPromptTagCatalog();
    this.#api = mountEditor(this.#content, {
      source: options.source,
      menuPresentation: (root) => new SugarPromptMenu(root, options.report),
      tooltipTransport: (plane) => new SugarPromptTooltip(plane),
      label: options.label,
      placeholder: 'Write a prompt…',
      readOnly: options.readOnly,
      richRendering: true,
      featureProfile: profile,
      tagCompletionCatalog: tagCatalog,
      tagLexicon: tagCatalog.lexicon,
      onUpdate: (snapshot) => this.#observe(snapshot),
      onLayout: (metrics) => this.#layout(metrics),
      onLoraMediaError: options.report,
    });
    markComfyPromptInput(this.element);
  }

  /** Preserve the native document and history when the host moves the same field to another presentation. */
  attach(textarea: HTMLTextAreaElement): void {
    const parent = textarea.parentElement;
    if (!parent || parent === this.element.parentElement) return;
    relocateControl(this.element, parent, textarea);
  }

  /** Adopt an external workflow value without normalizing stored source or echoing another widget edit. */
  refresh(source: string, readOnly: boolean): void {
    if (this.#disposed) return;
    this.#field?.setReadOnly(readOnly);
    const snapshot = this.#api.snapshot();
    if (source === this.#committedSource && readOnly === snapshot.readOnly) {
      this.#pendingRefresh = null;
      return;
    }
    this.#pendingRefresh = { source, readOnly };
    this.#queueRefresh();
  }

  /** Replace one catalog lifetime; obsolete resource replies cannot update mounted chips. */
  setLoraCatalog(adapter: LoraResourceAdapter): void {
    if (this.#disposed || this.#adapter === adapter) return;
    const generation = ++this.#resourceGeneration;
    this.#resources?.dispose();
    this.#adapter = adapter;
    this.#resourceKey = '';
    this.#resourceSnapshot = null;
    this.#resourceRevision = -1;
    this.#inspectionRevision = -1;
    this.#resources = new LoraResourceSession(adapter, (snapshot) => {
      if (generation === this.#resourceGeneration) this.#resourceChanged(snapshot);
    });
    this.#queueResources();
  }

  /** Expose a source observation for host diagnostics and behavioral tests. */
  snapshot(): EditorSnapshot {
    return this.#api.snapshot();
  }

  /** Complete construction after the native editor publishes its mounted layout. */
  whenPresented(): Promise<void> {
    return this.#readiness.wait();
  }

  /** Release the exact native view, thumbnail decoders and outstanding catalog work. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#readiness.dispose();
    this.#stopAppearance();
    this.#resources?.dispose();
    this.#api.destroy();
    this.element.remove();
  }

  /** Publish committed changes only; selection and unfinished preedit never write workflow values. */
  #observe(snapshot: EditorSnapshot): void {
    markComfyPromptInput(this.element);
    this.#snapshot = snapshot;
    if (!snapshot.composing && !this.#publishing && !this.#pendingRefresh) {
      this.#committedSource = snapshot.source;
      this.#options.commit(snapshot.source);
    }
    if (this.#pendingRefresh) this.#queueRefresh();
    this.#queueResources();
  }

  /** Adopt external state after native dispatch, preserving newer host intent during unfinished composition. */
  #queueRefresh(): void {
    if (this.#refreshQueued || this.#disposed) return;
    this.#refreshQueued = true;
    queueMicrotask(() => {
      this.#refreshQueued = false;
      if (this.#disposed || !this.#pendingRefresh) return;
      const { source, readOnly } = this.#pendingRefresh;
      let snapshot = this.#api.snapshot();
      if (snapshot.composing && readOnly !== snapshot.readOnly) {
        this.#api.setReadOnly(readOnly);
        snapshot = this.#api.snapshot();
      }
      if (snapshot.composing) return;
      this.#pendingRefresh = null;
      this.#committedSource = source;
      if (source === snapshot.source && readOnly === snapshot.readOnly) return;
      this.#publishing = true;
      try {
        this.#api.updateHost(
          source === snapshot.source
            ? { mode: 'preserve', configuration: { readOnly } }
            : { mode: 'baseline', source, configuration: { readOnly } },
        );
      } finally {
        this.#publishing = false;
      }
    });
  }

  /** Coalesce semantic settlement outside native dispatch before preparing parsed resource names. */
  #queueResources(): void {
    if (this.#scheduled || this.#disposed) return;
    this.#scheduled = true;
    queueMicrotask(() => {
      this.#scheduled = false;
      const snapshot = this.#snapshot;
      if (
        this.#disposed ||
        !this.#resources ||
        !snapshot ||
        snapshot.composing ||
        snapshot.semantics !== 'ready'
      )
        return;
      this.#resourceChanged(this.#resources.snapshot());
      if (this.#inspectionRevision === snapshot.revision) return;
      const inspected = this.#api.inspectStructure();
      if (inspected.status !== 'ready') return;
      this.#inspectionRevision = snapshot.revision;
      const key = JSON.stringify(inspected.loraResourceNames);
      if (key === this.#resourceKey) return;
      this.#resourceKey = key;
      void this.#resources.warmupNames(inspected.loraResourceNames).catch(this.#options.report);
    });
  }

  /** Publish resource snapshots only against the current committed native source revision. */
  #resourceChanged(resources: LoraResourceSnapshot): void {
    if (this.#disposed || ['idle', 'loading', 'disposed'].includes(resources.status)) return;
    const snapshot = this.#api.snapshot();
    if (snapshot.composing) return;
    if (resources === this.#resourceSnapshot && snapshot.revision === this.#resourceRevision)
      return;
    if (this.#api.setLoraResources(resources, snapshot.revision)) {
      this.#resourceSnapshot = resources;
      this.#resourceRevision = snapshot.revision;
    }
  }

  /** Follow the accepted field design system without recreating the editable document. */
  #applyAppearance(): void {
    const { style, dark } = this.#options.appearance.appearance;
    this.element.dataset.scPromptStyle = style;
    if (style === 'photopro') this.element.style.removeProperty('color-scheme');
    else this.element.style.colorScheme = dark ? 'dark' : 'light';
    if (style === 'comfy') applyOwnedTheme(this.element, null);
    else applyOwnedTheme(this.element, dark, style);
    if (this.#fieldStyle !== style) {
      const previous = this.#field;
      const field = createEditorField(this.element.ownerDocument, style);
      this.element.append(field.root);
      field.attachEditor(this.#content);
      field.setReadOnly(this.#api?.snapshot().readOnly ?? this.#options.readOnly);
      previous?.root.remove();
      this.#field = field;
      this.#fieldStyle = style;
    }
  }

  /** Preserve native CSS-local geometry under both Comfy canvas transforms and ordinary masonry layout. */
  #layout(metrics: EditorLayoutMetrics): void {
    if (this.#disposed) return;
    const height = Math.max(
      128,
      Math.min(320, Math.ceil(metrics.contentHeight + metrics.outerPadding)),
    );
    if (this.#allocatedHeight !== height) {
      this.#allocatedHeight = height;
      this.element.style.height = `${String(height)}px`;
      this.#options.layout(height);
    }
    this.#readiness.publish(metrics.viewportWidth);
  }
}
