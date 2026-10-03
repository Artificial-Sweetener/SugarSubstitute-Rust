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
import type { UnknownRecord } from '../../types/common.js';
import {
  createComfyRendererModeChangeSource,
  resolveComfyRendererMode,
} from '../../core/ComfyRendererMode.js';
import { NativeFluentCardAdapter } from './NativeFluentCardAdapter.js';
import { fluentAppearanceCss } from './FluentAppearanceStyles.js';
import { applyOwnedFluentTheme, createFluentNumericSlider } from './FluentThemeRuntime.js';
import { NativeInlineComboSearch } from './NativeInlineComboSearch.js';
import { NativeCardTitlebar } from './NativeCardTitlebar.js';
import { NativeToggleRows } from './NativeToggleRows.js';
import { NativeAdvancedDisclosure } from './NativeAdvancedDisclosure.js';
import { NativeNumericControls } from './NativeNumericControls.js';
import { NativeLabelPresentation } from './NativeLabelPresentation.js';

/** Persist the user's appearance preference independently of workflow documents. */
export const FLUENT_APPEARANCE_SETTING = 'SugarCubes.Appearance.Fluent';
const surfaceSelector = '.sugarcubes-workflow-masonry, .sugarcubes-masonry-toggle';

export interface FluentAppearanceHostOptions {
  document: Document;
  app: unknown;
  reportError(error: unknown): void;
}

/** Keep appearance separate from graph, Cube policy, and masonry layout state. */
export class FluentAppearanceHost {
  readonly #options: FluentAppearanceHostOptions;
  readonly #adapter = new NativeFluentCardAdapter();
  readonly #titlebars = new NativeCardTitlebar();
  readonly #labels = new NativeLabelPresentation();
  readonly #toggleRows = new NativeToggleRows();
  readonly #advanced = new NativeAdvancedDisclosure();
  readonly #numeric = new NativeNumericControls(createFluentNumericSlider);
  readonly #comboSearch: NativeInlineComboSearch;
  readonly #style: HTMLStyleElement;
  readonly #presence: MutationObserver;
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
    this.#comboSearch = new NativeInlineComboSearch(options.document);
    this.#style = options.document.createElement('style');
    this.#style.id = 'sugarcubes-fluent-appearance';
    this.#presence = new MutationObserver((records) => {
      if (!this.#active) return;
      for (const record of records) {
        if (record.target instanceof HTMLElement) this.#queue(record.target);
        else if (record.target instanceof Text && record.target.parentElement)
          this.#queue(record.target.parentElement);
        for (const node of record.addedNodes) if (node instanceof HTMLElement) this.#queue(node);
      }
      this.#schedule();
    });
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
    this.#presence.observe(this.#options.document.body, {
      childList: true,
      characterData: true,
      subtree: true,
    });
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
    this.#presence.disconnect();
    this.#palette.disconnect();
    this.#unsubscribe?.();
    this.#unsubscribePalette?.();
    this.#options.document.removeEventListener('pointerdown', this.#rememberPopupOrigin, true);
    this.#options.document.removeEventListener('keydown', this.#rememberPopupOrigin, true);
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
    this.#queue(this.#options.document.body);
    this.#schedule();
  }

  /** Expose the underlying host theme, which remains live throughout the session. */
  #deactivate(): void {
    this.#active = false;
    this.#style.remove();
    this.#adapter.clear();
    this.#labels.clear();
    this.#titlebars.clear();
    this.#toggleRows.clear();
    this.#advanced.clear();
    this.#numeric.clear();
    this.#comboSearch.clear();
    for (const surface of this.#surfaces) {
      applyOwnedFluentTheme(surface, null);
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
    const scheme = this.#options.document.defaultView?.getComputedStyle(
      this.#options.document.documentElement,
    ).colorScheme;
    const dark =
      scheme === 'dark' ||
      (scheme !== 'light' && !this.#options.document.querySelector('.light-theme'));
    if (dark === this.#dark) return;
    this.#dark = dark;
    this.#style.textContent = fluentAppearanceCss(dark);
    for (const surface of this.#surfaces) applyOwnedFluentTheme(surface, dark);
  }

  /** Discover changed subtrees and their nearest card rather than scanning all cards per keystroke. */
  #queue(element: HTMLElement): void {
    const root = element.closest<HTMLElement>('.lg-node');
    if (root) this.#dirty.add(root);
    if (element.matches('.lg-node') || element.matches(surfaceSelector)) this.#dirty.add(element);
    for (const node of element.querySelectorAll<HTMLElement>(`.lg-node, ${surfaceSelector}`))
      this.#dirty.add(node);
  }

  /** Coalesce native discovery and report unsupported host failures through SugarCubes feedback. */
  #schedule(): void {
    if (this.#frame !== null || !this.#active) return;
    const run = (): void => {
      this.#frame = null;
      if (!this.#active) return;
      try {
        for (const surface of this.#surfaces)
          if (!surface.isConnected) this.#surfaces.delete(surface);
        for (const popup of this.#popups) if (!popup.isConnected) this.#popups.delete(popup);
        this.#comboSearch.reconcile();
        this.#annotatePopup();
        if (
          [...this.#dirty].some(
            (element) =>
              element.matches('.lg-node') && !element.closest('.sugarcubes-native-node-card'),
          )
        )
          this.#adapter.beginBatch(this.#options.document);
        for (const element of this.#dirty) {
          if (!element.isConnected) continue;
          if (element.matches('.lg-node')) {
            const card = this.#adapter.annotate(element);
            this.#labels.reconcile(element, card.labelElements);
            this.#titlebars.reconcile(element, card.header, card.collapsed, card.canCollapse);
            this.#toggleRows.reconcile(element);
            this.#advanced.reconcile(card);
            this.#numeric.reconcile(element, card.numericFields);
          } else {
            element.setAttribute('data-sc-fluent-surface', '');
            if (!this.#surfaces.has(element)) applyOwnedFluentTheme(element, this.#dark ?? true);
            this.#surfaces.add(element);
          }
        }
        this.#dirty.clear();
        this.#adapter.endBatch();
      } catch (error: unknown) {
        this.#options.reportError(error);
        this.#deactivate();
      }
    };
    const windowRef = this.#options.document.defaultView;
    if (windowRef?.requestAnimationFrame) this.#frame = windowRef.requestAnimationFrame(run);
    else {
      this.#frame = -1;
      queueMicrotask(run);
    }
  }

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
