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
import { NativeThemeScopes } from './NativeThemeScopes.js';
import { identifyMenuSurface, releaseMenuSurface } from '../../menus/MenuSurface.js';
import { nativeVisualStyleCss } from './ThemeService.js';
import { NativeFieldAppearance } from './NativeFieldAppearance.js';
import { NativeInlineComboSearch } from './NativeInlineComboSearch.js';
import { acquireComboToggle } from './ComboToggleController.js';
import type { NativeSeedReuseResolver } from './ComfyNativeSeedControl.js';
import { readOwnedDarkTheme } from './OwnedColorScheme.js';
import { OverlayScrollbarHost } from './OverlayScrollbarHost.js';
import { isBorrowedNativeCardPresentation } from '../NativeCardPresentationOwner.js';
import { NATIVE_PRESENTATION_READY_EVENT } from '../NativePresentationReady.js';
import { NativeCardGraphGeometry } from './NativeCardGraphGeometry.js';
import { AppearanceState } from './AppearanceState.js';
import { AccentThemeHost } from './AccentThemeHost.js';
import { CubeIconAppearanceHost } from './CubeIconAppearanceHost.js';
import { ensureOwnedControlAppearanceStyles } from './OwnedControlAppearanceStyles.js';
import { refreshCubeTitleOpticalAlignment } from '../CubeTitleOpticalDomAlignment.js';
import type { VisualStyle } from './VisualStyle.js';
import { registerVisualStylePreference } from './VisualStylePreference.js';
import { publishVisualStyle, releaseVisualStyle } from './VisualStyleBoundary.js';
import { registerCompactMasonryPreference } from '../masonry/CompactMasonryPreference.js';
import { registerPhotoProPalettePreference } from './PhotoProPalettePreference.js';
import type { EmbeddedPresentationSource } from '../../core/embedded/EmbeddedPresentation.js';

const surfaceSelector =
  '.sugarcubes-workflow-masonry, .sugarcubes-masonry-toggle, .sc-overrides-toolbar, .sugarcubes-native-widget';

export interface AppearanceHostOptions {
  document: Document;
  app: unknown;
  embedded?: EmbeddedPresentationSource;
  seedReuse?: NativeSeedReuseResolver;
  getSlotLayoutSync?(): Promise<() => void>;
  reportError(error: unknown): void;
}

/** Keep appearance separate from graph, Cube policy, and masonry layout state. */
export class AppearanceHost {
  readonly state = new AppearanceState();
  readonly #options: AppearanceHostOptions;
  readonly #nodes: NativeFieldAppearance;
  readonly #widgets: NativeFieldAppearance;
  #nativeCards = false;
  readonly #graphGeometry: NativeCardGraphGeometry;
  readonly #themes = new NativeThemeScopes();
  readonly #comboSearch: NativeInlineComboSearch;
  readonly #scrollbars: OverlayScrollbarHost;
  readonly #style: HTMLStyleElement;
  readonly #accent: AccentThemeHost;
  readonly #iconAppearance: CubeIconAppearanceHost;
  #presence: HostSurfaceObserver | null = null;
  readonly #palette: MutationObserver;
  readonly #surfaces = new Set<HTMLElement>();
  readonly #popups = new Set<HTMLElement>();
  readonly #dirty = new Set<HTMLElement>();
  #unsubscribe: (() => void) | null = null;
  #unsubscribePalette: (() => void) | null = null;
  #releaseComboToggle: (() => void) | null = null;
  #releaseCompactMasonry: (() => void) | null = null;
  #releasePhotoProPalette: (() => void) | null = null;
  #releaseVisualStyle: (() => void) | null = null;
  #selectedStyle: VisualStyle = 'comfy';
  #active = false;
  #dark: boolean | null = null;
  #frame: number | null = null;
  #disposed = false;
  #setup = false;
  #popupOrigin: HTMLElement | null = null;

  /** Share the accepted palette while keeping frame mode independent of node rendering. */
  get palette(): AccentThemeHost {
    return this.#accent;
  }

  /** Prepare observers without altering styles until the user opts in. */
  constructor(options: AppearanceHostOptions) {
    this.#options = options;
    this.#accent = new AccentThemeHost(
      options.document,
      options.app,
      options.reportError,
      options.embedded,
    );
    this.#iconAppearance = new CubeIconAppearanceHost(options.document, this.state);
    this.#scrollbars = new OverlayScrollbarHost(options.document, options.reportError);
    this.#nodes = new NativeFieldAppearance(options.app, 'card', options.seedReuse);
    this.#widgets = new NativeFieldAppearance(options.app, 'widget', options.seedReuse);
    this.#graphGeometry = new NativeCardGraphGeometry({
      document: options.document,
      ...(options.getSlotLayoutSync ? { getSynchronizer: options.getSlotLayoutSync } : {}),
      reportError: options.reportError,
    });
    this.#comboSearch = new NativeInlineComboSearch(options.document);
    this.#style = options.document.createElement('style');
    this.#style.id = 'sugarcubes-native-appearance';
    this.#palette = new MutationObserver(() => this.#refreshPalette());
  }

  /** Register host persistence and renderer/theme subscriptions through their existing owners. */
  setup(): void {
    if (this.#setup || this.#disposed) return;
    this.#setup = true;
    this.#releaseComboToggle = acquireComboToggle(this.#options.document);
    ensureOwnedControlAppearanceStyles(this.#options.document);
    this.#accent.setup();
    const settings = readSettings(this.#options.app);
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
        exclude: '.sc-style-scrollbar-layer,[data-sc-scrollbar-policy]',
      },
    );
    for (const element of [
      this.#options.document.documentElement,
      this.#options.document.body,
      this.#options.document.getElementById('vue-app'),
    ]) {
      if (element)
        this.#palette.observe(element, {
          attributes: true,
          attributeFilter: ['class', 'style', 'data-sc-visual-style', 'data-sc-photopro-palette'],
        });
    }
    this.#options.document.addEventListener('pointerdown', this.#rememberPopupOrigin, true);
    this.#options.document.addEventListener('keydown', this.#rememberPopupOrigin, true);
    this.#options.document.addEventListener(
      NATIVE_PRESENTATION_READY_EVENT,
      this.#onPresentationReady,
    );
    this.#options.document.defaultView?.addEventListener('pagehide', this.#onPageHide);
    this.#releaseVisualStyle = registerVisualStylePreference(
      this.#options.app,
      (style) => this.setStyle(style),
      this.#options.reportError,
      this.#options.embedded,
    );
    this.#releaseCompactMasonry = registerCompactMasonryPreference(
      this.#options.app,
      this.#options.document,
      this.#options.reportError,
    );
    this.#releasePhotoProPalette = registerPhotoProPalettePreference(
      this.#options.app,
      this.#options.document,
      this.#options.reportError,
      this.#options.embedded,
    );
  }

  /** Apply preference without persisting presentation into workflow node data. */
  setStyle(style: VisualStyle): void {
    if (this.#disposed) return;
    if (style !== this.#selectedStyle) this.#deactivate(false);
    this.#selectedStyle = style;
    publishVisualStyle(this.#options.document, style);
    this.#reconcile();
  }

  /** Release owned styles and annotations without restoring stale host values. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#accent.dispose();
    this.#iconAppearance.dispose();
    this.#deactivate();
    releaseVisualStyle(this.#options.document, this.#selectedStyle);
    this.state.dispose();
    this.#presence?.disconnect();
    this.#palette.disconnect();
    this.#unsubscribe?.();
    this.#unsubscribePalette?.();
    this.#releaseComboToggle?.();
    this.#releaseCompactMasonry?.();
    this.#releasePhotoProPalette?.();
    this.#releaseVisualStyle?.();
    this.#options.document.removeEventListener('pointerdown', this.#rememberPopupOrigin, true);
    this.#options.document.removeEventListener('keydown', this.#rememberPopupOrigin, true);
    this.#options.document.removeEventListener(
      NATIVE_PRESENTATION_READY_EVENT,
      this.#onPresentationReady,
    );
    this.#options.document.defaultView?.removeEventListener('pagehide', this.#onPageHide);
  }

  /** Keep owned widgets styled while native-card presentation follows its graph renderer. */
  #reconcile(): void {
    if (this.#disposed) return;
    const themed = this.#selectedStyle !== 'comfy';
    const nativeCards =
      themed && resolveComfyRendererMode(this.#options.app, null, this.#options.document) === 'vue';
    if (this.#nativeCards && !nativeCards) {
      this.#nodes.clear();
      this.#scrollbars.clear();
      this.#graphGeometry.clear();
    }
    this.#nativeCards = nativeCards;
    this.#active = true;
    if (themed && !this.#style.isConnected) this.#options.document.head.append(this.#style);
    this.#refreshPalette();
    ensureOwnedControlAppearanceStyles(this.#options.document);
    if (themed) this.#scrollbars.enable();
    this.#queue(this.#options.document.body);
    this.#schedule();
    refreshCubeTitleOpticalAlignment(this.#options.document);
  }

  /** Expose the underlying host theme, which remains live throughout the session. */
  #deactivate(publish = true): void {
    const synchronizeRestored =
      this.#active &&
      !this.#disposed &&
      resolveComfyRendererMode(this.#options.app, null, this.#options.document) === 'vue';
    this.#active = false;
    if (publish) this.state.update('comfy', readOwnedDarkTheme(this.#options.document));
    this.#style.remove();
    this.#scrollbars.clear();
    this.#nodes.clear();
    this.#widgets.clear();
    this.#nativeCards = false;
    this.#themes.clear();
    if (synchronizeRestored) this.#graphGeometry.stylesRemoved();
    else this.#graphGeometry.clear();
    this.#comboSearch.clear();
    for (const surface of this.#surfaces) {
      surface.removeAttribute('data-sc-style-surface');
      surface.removeAttribute('data-sc-style-popup');
      releaseMenuSurface(surface);
    }
    this.#surfaces.clear();
    for (const popup of this.#popups) {
      popup.removeAttribute('data-sc-style-popup');
      releaseMenuSurface(popup);
    }
    this.#popups.clear();
    this.#popupOrigin = null;
    this.#dirty.clear();
    this.#dark = null;
    if (this.#frame !== null) this.#options.document.defaultView?.cancelAnimationFrame(this.#frame);
    this.#frame = null;
  }

  /** Publish accepted surface brightness without remounting themed controls. */
  #refreshPalette(): void {
    const dark = readOwnedDarkTheme(this.#options.document);
    this.state.update(this.#active && this.#nativeCards ? this.#selectedStyle : 'comfy', dark);
    if (!this.#active) return;
    if (dark === this.#dark) return;
    this.#dark = dark;
    if (this.#selectedStyle !== 'comfy')
      this.#style.textContent = nativeVisualStyleCss(this.#selectedStyle, dark);
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
    return element.hasAttribute('data-sc-style-node') && isBorrowedNativeCardPresentation(element);
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
      const themed = this.#selectedStyle !== 'comfy';
      if (themed) {
        this.#comboSearch.reconcile();
        this.#annotatePopup();
      }
      if (this.#nativeCards && [...this.#dirty].some((element) => element.matches('.lg-node')))
        this.#nodes.beginBatch(
          this.#options.document,
          [...this.#dirty].filter((element) => element.matches('.lg-node')),
        );
      for (const element of this.#dirty) {
        if (!element.isConnected) continue;
        if (themed) this.#themes.retain(element);
        if (element.matches('.lg-node, .sugarcubes-native-widget')) {
          const standalone = element.matches('.sugarcubes-native-widget');
          if (!standalone && !this.#nativeCards && element.dataset.cubeFacePresentation !== 'true')
            continue;
          (standalone ? this.#widgets : this.#nodes).present(element, themed);
          if (!standalone)
            for (const widget of element.querySelectorAll<HTMLElement>('.sugarcubes-native-widget'))
              this.#dirty.add(widget);
          if (!standalone && this.#nativeCards) this.#graphGeometry.request(element);
        } else if (themed) {
          if (!element.hasAttribute('data-sc-style-surface'))
            element.setAttribute('data-sc-style-surface', '');
          this.#surfaces.add(element);
        }
      }
      this.#dirty.clear();
      this.#nodes.endBatch();
      this.#widgets.endBatch();
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
    if (target.matches('.sugarcubes-native-widget')) {
      this.#dirty.add(target);
      this.#schedule();
      return;
    }
    this.#presence?.flushPending();
    this.#queue(target);
    if (this.#frame !== null) this.#options.document.defaultView?.cancelAnimationFrame(this.#frame);
    this.#frame = null;
    this.#flush();
  };

  /** Associate portalled menus with their native source field rather than all host popups. */
  readonly #rememberPopupOrigin = (event: Event): void => {
    if (event.target instanceof HTMLElement && event.target.closest('[data-sc-style-popup]'))
      return;
    this.#popupOrigin =
      event.target instanceof HTMLElement
        ? event.target.closest<HTMLElement>('[data-sc-style-row],.sugarcubes-native-widget')
        : null;
  };

  /** Follow the source control's accessibility relationship to its own portalled menu. */
  #annotatePopup(): void {
    if (!this.#popupOrigin?.isConnected) return;
    for (const trigger of this.#popupOrigin.querySelectorAll('[aria-controls]')) {
      const id = trigger.getAttribute('aria-controls');
      const popup = id ? this.#options.document.getElementById(id) : null;
      if (!popup || popup.getAttribute('role') !== 'listbox' || popup.closest('.sc-model-popup'))
        continue;
      popup.setAttribute('data-sc-style-popup', '');
      identifyMenuSurface(popup, 'choices');
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
