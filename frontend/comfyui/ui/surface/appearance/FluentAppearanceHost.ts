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
/** Own the reversible appearance setting and native presentation lifetime. */
import { isRecord } from '../../types/common.js';
import { HostSurfaceObserver } from '../../core/HostSurfaceObserver.js';
import type { UnknownRecord } from '../../types/common.js';
import {
  createComfyRendererModeChangeSource,
  resolveComfyRendererMode,
} from '../../core/ComfyRendererMode.js';
import { NativeFluentThemeScopes } from './NativeFluentThemeScopes.js';
import { NativeFluentCardAdapter } from './NativeFluentCardAdapter.js';
import { fluentAppearanceCss } from './FluentAppearanceStyles.js';
import { createFluentNumericSlider, createFluentBooleanCheckbox } from './FluentThemeRuntime.js';
import { NativeInlineComboSearch } from './NativeInlineComboSearch.js';
import { NativeCardTitlebar } from './NativeCardTitlebar.js';
import { NativeToggleRows } from './NativeToggleRows.js';
import { NativeAdvancedDisclosure } from './NativeAdvancedDisclosure.js';
import { NativeNumericControls } from './NativeNumericControls.js';
import { NativeLabelPresentation } from './NativeLabelPresentation.js';
import { NativeSeedControls } from './NativeSeedControls.js';
import { readComfyDarkTheme } from './ComfyColorScheme.js';
import { FluentOverlayScrollbarHost } from './FluentOverlayScrollbarHost.js';
import { isBorrowedNativeCardPresentation } from '../NativeCardPresentationOwner.js';
import { NATIVE_PRESENTATION_READY_EVENT } from '../NativePresentationReady.js';
import { NativeFluentGraphGeometry } from './NativeFluentGraphGeometry.js';

/** Persist the user's appearance preference independently of workflow documents. */
export const FLUENT_APPEARANCE_SETTING = 'SugarCubes.Appearance.Fluent';
const surfaceSelector =
  '.sugarcubes-workflow-masonry, .sugarcubes-masonry-toggle, .sc-overrides-toolbar, .sugarcubes-native-widget';

export interface FluentAppearanceHostOptions {
  document: Document;
  app: unknown;
  getSlotLayoutSync?(): Promise<() => void>;
  reportError(error: unknown): void;
}

/** Keep appearance separate from graph, Cube policy, and masonry layout state. */
export class FluentAppearanceHost {
  readonly #options: FluentAppearanceHostOptions;
  readonly #adapter: NativeFluentCardAdapter;
  readonly #graphGeometry: NativeFluentGraphGeometry;
  readonly #themes = new NativeFluentThemeScopes();
  readonly #titlebars = new NativeCardTitlebar();
  readonly #labels = new NativeLabelPresentation();
  readonly #toggleRows = new NativeToggleRows(createFluentBooleanCheckbox);
  readonly #advanced = new NativeAdvancedDisclosure();
  readonly #numeric = new NativeNumericControls(createFluentNumericSlider);
  readonly #seeds = new NativeSeedControls();
  readonly #comboSearch: NativeInlineComboSearch;
  readonly #scrollbars: FluentOverlayScrollbarHost;
  readonly #style: HTMLStyleElement;
  #presence: HostSurfaceObserver | null = null;
  readonly #palette: MutationObserver;
  readonly #surfaces = new Set<HTMLElement>();
  readonly #popups = new Set<HTMLElement>();
  readonly #dirty = new Set<HTMLElement>();
  #unsubscribe: (() => void) | null = null;
  #unsubscribePalette: (() => void) | null = null;
  #enabled = false;
  #active = false;
  #dark: boolean | null = null;
  #frame: number | null = null;
  #disposed = false;
  #setup = false;
  #popupOrigin: HTMLElement | null = null;

  /** Prepare observers without altering styles until the user opts in. */
  constructor(options: FluentAppearanceHostOptions) {
    this.#options = options;
    this.#scrollbars = new FluentOverlayScrollbarHost(options.document, options.reportError);
    this.#adapter = new NativeFluentCardAdapter(options.app);
    this.#graphGeometry = new NativeFluentGraphGeometry({
      document: options.document,
      ...(options.getSlotLayoutSync ? { getSynchronizer: options.getSlotLayoutSync } : {}),
      reportError: options.reportError,
    });
    this.#comboSearch = new NativeInlineComboSearch(options.document);
    this.#style = options.document.createElement('style');
    this.#style.id = 'sugarcubes-fluent-appearance';
    this.#palette = new MutationObserver(() => this.#refreshPalette());
  }

  /** Register host persistence and renderer/theme subscriptions through their existing owners. */
  setup(): void {
    if (this.#setup || this.#disposed) return;
    this.#setup = true;
    const settings = readSettings(this.#options.app);
    if (typeof settings.addSetting !== 'function') {
      this.#options.reportError(
        new Error('Comfy settings registration is unavailable for Fluent appearance.'),
      );
      return;
    }
    settings.addSetting.call(settings, {
      id: FLUENT_APPEARANCE_SETTING,
      name: 'Fluent theme',
      category: ['SugarCubes', 'Appearance', 'Fluent'],
      type: 'boolean',
      defaultValue: false,
      tooltip: 'Use Windows Fluent styling and SugarSubstitute card presentation in Nodes 2.0.',
      onChange: (value: unknown) => this.setEnabled(value === true),
    });
    this.#unsubscribe = createComfyRendererModeChangeSource(this.#options.app).subscribe(() =>
      this.#reconcile(),
    );
    if (
      typeof settings.addEventListener === 'function' &&
      typeof settings.removeEventListener === 'function'
    ) {
      const removePaletteListener = settings.removeEventListener;
      const refresh: EventListener = () => queueMicrotask(() => this.#refreshPalette());
      settings.addEventListener.call(settings, 'Comfy.ColorPalette.change', refresh);
      this.#unsubscribePalette = () =>
        removePaletteListener.call(settings, 'Comfy.ColorPalette.change', refresh);
    }
    this.#presence = new HostSurfaceObserver(
      this.#options.document.body,
      `.lg-node, ${surfaceSelector}, [role="listbox"]`,
      (changes) => {
        if (!this.#active) return;
        for (const { element, kind, relocated } of changes)
          if (
            element.getAttribute('role') !== 'listbox' &&
            !(relocated && kind === 'mounted' && this.#retainedCard(element))
          )
            this.#dirty.add(element);
        this.#schedule();
      },
      {
        content: true,
        characterData: true,
        exclude: '.sc-fluent-scrollbar-layer,[data-sc-scrollbar-policy]',
      },
    );
    for (const element of [
      this.#options.document.documentElement,
      this.#options.document.body,
      this.#options.document.getElementById('vue-app'),
    ]) {
      if (element)
        this.#palette.observe(element, { attributes: true, attributeFilter: ['class', 'style'] });
    }
    this.#options.document.addEventListener('pointerdown', this.#rememberPopupOrigin, true);
    this.#options.document.addEventListener('keydown', this.#rememberPopupOrigin, true);
    this.#options.document.addEventListener(
      NATIVE_PRESENTATION_READY_EVENT,
      this.#onPresentationReady,
    );
    this.#options.document.defaultView?.addEventListener('pagehide', this.#onPageHide);
    const value: unknown =
      typeof settings.getSettingValue === 'function'
        ? settings.getSettingValue.call(settings, FLUENT_APPEARANCE_SETTING)
        : false;
    this.setEnabled(value === true);
  }

  /** Apply preference without persisting presentation into workflow node data. */
  setEnabled(enabled: boolean): void {
    this.#enabled = enabled;
    this.#reconcile();
  }

  /** Release owned styles and annotations without restoring stale host values. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#deactivate();
    this.#presence?.disconnect();
    this.#palette.disconnect();
    this.#unsubscribe?.();
    this.#unsubscribePalette?.();
    this.#options.document.removeEventListener('pointerdown', this.#rememberPopupOrigin, true);
    this.#options.document.removeEventListener('keydown', this.#rememberPopupOrigin, true);
    this.#options.document.removeEventListener(
      NATIVE_PRESENTATION_READY_EVENT,
      this.#onPresentationReady,
    );
    this.#options.document.defaultView?.removeEventListener('pagehide', this.#onPageHide);
  }

  /** Suspend under Nodes 1 while retaining the preference for returning to Nodes 2. */
  #reconcile(): void {
    if (this.#disposed) return;
    const active =
      this.#enabled &&
      resolveComfyRendererMode(this.#options.app, null, this.#options.document) === 'vue';
    if (!active) {
      this.#deactivate();
      return;
    }
    this.#active = true;
    if (!this.#style.isConnected) this.#options.document.head.append(this.#style);
    this.#refreshPalette();
    this.#scrollbars.enable();
    this.#queue(this.#options.document.body);
    this.#schedule();
  }

  /** Expose the underlying host theme, which remains live throughout the session. */
  #deactivate(): void {
    const synchronizeRestored =
      this.#active &&
      !this.#disposed &&
      resolveComfyRendererMode(this.#options.app, null, this.#options.document) === 'vue';
    this.#active = false;
    this.#style.remove();
    this.#scrollbars.clear();
    this.#adapter.clear();
    this.#themes.clear();
    this.#labels.clear();
    this.#titlebars.clear();
    this.#toggleRows.clear();
    this.#advanced.clear();
    this.#numeric.clear();
    this.#seeds.clear();
    if (synchronizeRestored) this.#graphGeometry.stylesRemoved();
    else this.#graphGeometry.clear();
    this.#comboSearch.clear();
    for (const surface of this.#surfaces) {
      surface.removeAttribute('data-sc-fluent-surface');
      surface.removeAttribute('data-sc-fluent-popup');
    }
    this.#surfaces.clear();
    for (const popup of this.#popups) popup.removeAttribute('data-sc-fluent-popup');
    this.#popups.clear();
    this.#popupOrigin = null;
    this.#dirty.clear();
    this.#dark = null;
    if (this.#frame !== null) this.#options.document.defaultView?.cancelAnimationFrame(this.#frame);
    this.#frame = null;
  }

  /** Follow Comfy's theme selector without replacing its palette owner. */
  #refreshPalette(): void {
    if (!this.#active) return;
    const dark = readComfyDarkTheme(this.#options.document);
    if (dark === this.#dark) return;
    this.#dark = dark;
    this.#style.textContent = fluentAppearanceCss(dark);
  }

  /** Discover changed subtrees and their nearest card rather than scanning all cards per keystroke. */
  #queue(element: HTMLElement): void {
    const root = element.closest<HTMLElement>('.lg-node, .sugarcubes-native-widget');
    if (root) this.#dirty.add(root);
    if (element.matches('.lg-node') || element.matches(surfaceSelector)) this.#dirty.add(element);
    for (const node of element.querySelectorAll<HTMLElement>(`.lg-node, ${surfaceSelector}`))
      if (!this.#retainedCard(node)) this.#dirty.add(node);
  }

  /** Retain already styled controls only while their exact source renderer owns the loan. */
  #retainedCard(element: HTMLElement): boolean {
    return element.hasAttribute('data-sc-fluent-node') && isBorrowedNativeCardPresentation(element);
  }

  /** Coalesce native discovery and report unsupported host failures through SugarCubes feedback. */
  #schedule(): void {
    if (this.#frame !== null || !this.#active) return;
    const run = (): void => {
      this.#frame = null;
      this.#flush();
    };
    const windowRef = this.#options.document.defaultView;
    if (windowRef?.requestAnimationFrame) this.#frame = windowRef.requestAnimationFrame(run);
    else {
      this.#frame = -1;
      queueMicrotask(run);
    }
  }

  /** Finish identified presentation work before a caller measures native geometry. */
  #flush(): void {
    if (!this.#active) return;
    try {
      for (const surface of this.#surfaces)
        if (!surface.isConnected) this.#surfaces.delete(surface);
      for (const popup of this.#popups) if (!popup.isConnected) this.#popups.delete(popup);
      this.#themes.prune();
      this.#comboSearch.reconcile();
      this.#annotatePopup();
      if ([...this.#dirty].some((element) => element.matches('.lg-node')))
        this.#adapter.beginBatch(
          this.#options.document,
          [...this.#dirty].filter((element) => element.matches('.lg-node')),
        );
      for (const element of this.#dirty) {
        if (!element.isConnected) continue;
        this.#themes.retain(element);
        if (element.matches('.lg-node, .sugarcubes-native-widget')) {
          const standalone = element.matches('.sugarcubes-native-widget');
          const card = standalone
            ? this.#adapter.annotateWidget(element)
            : this.#adapter.annotate(element);
          this.#labels.reconcile(element, card.labelElements);
          if (!standalone)
            this.#titlebars.reconcile(element, card.header, card.collapsed, card.canCollapse);
          this.#toggleRows.reconcile(element);
          this.#advanced.reconcile(card);
          this.#numeric.reconcile(element, card.numericFields);
          this.#seeds.reconcile(element, card.seedFields);
          if (!standalone) this.#graphGeometry.request(element);
        } else {
          if (!element.hasAttribute('data-sc-fluent-surface'))
            element.setAttribute('data-sc-fluent-surface', '');
          this.#surfaces.add(element);
        }
      }
      this.#dirty.clear();
      this.#adapter.endBatch();
      this.#presence?.discardPending();
    } catch (error: unknown) {
      this.#options.reportError(error);
      this.#deactivate();
    }
  }

  /** Settle a named mounted surface through the same authoritative appearance reconciliation. */
  readonly #onPresentationReady = (event: Event): void => {
    const target = event.target;
    if (
      !this.#active ||
      !(target instanceof HTMLElement) ||
      !target.matches(`.lg-node, ${surfaceSelector}`)
    )
      return;
    this.#presence?.flushPending();
    this.#queue(target);
    if (this.#frame !== null) this.#options.document.defaultView?.cancelAnimationFrame(this.#frame);
    this.#frame = null;
    this.#flush();
  };

  /** Associate portalled menus with their native source field rather than all host popups. */
  readonly #rememberPopupOrigin = (event: Event): void => {
    if (event.target instanceof HTMLElement && event.target.closest('[data-sc-fluent-popup]'))
      return;
    this.#popupOrigin =
      event.target instanceof HTMLElement
        ? event.target.closest<HTMLElement>('[data-sc-fluent-row]')
        : null;
  };

  /** Follow the source control's accessibility relationship to its own portalled menu. */
  #annotatePopup(): void {
    if (!this.#popupOrigin?.isConnected) return;
    for (const trigger of this.#popupOrigin.querySelectorAll('[aria-controls]')) {
      const id = trigger.getAttribute('aria-controls');
      const popup = id ? this.#options.document.getElementById(id) : null;
      if (!popup || popup.getAttribute('role') !== 'listbox') continue;
      popup.setAttribute('data-sc-fluent-popup', '');
      this.#themes.retain(popup);
      this.#popups.add(popup);
      if (trigger instanceof HTMLElement) this.#comboSearch.present(trigger, popup);
    }
  }

  /** Dispose before the host Vue application is released. */
  readonly #onPageHide = (): void => this.dispose();
}

/** Narrow the dynamic settings surface at the host boundary. */
function readSettings(app: unknown): UnknownRecord {
  const application = isRecord(app) ? app : {};
  const ui = isRecord(application.ui) ? application.ui : {};
  return isRecord(ui.settings) ? ui.settings : {};
}
