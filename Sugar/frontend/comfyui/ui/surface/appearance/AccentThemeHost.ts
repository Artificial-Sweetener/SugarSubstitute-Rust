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
/** Own accent persistence, external CSS overrides, and palette-only invalidation. */
import { ComfySettingStore } from '../../settings/workspace/ComfySettingStore.js';
import { settingsCatalogFor } from '../../settings/workspace/SettingsCatalog.js';
import { fixed } from '../../settings/workspace/SettingsModel.js';
import { isRecord, type UnknownRecord } from '../../types/common.js';
import { readOwnedDarkTheme } from './OwnedColorScheme.js';
import { DEFAULT_ACCENT_COLOR, normalizeAccentColor } from './AccentPalette.js';
import { createAccentColorControls, type AccentColorControls } from './AccentColorSetting.js';
import { HOST_CAPABILITIES } from '../../core/embedded/PanelRuntime.js';
import type { HostDescriptionSource } from '../../core/embedded/HostDescriptionState.js';
import type { EmbeddedPresentationSource } from '../../core/embedded/EmbeddedPresentation.js';
import type { HostAccentSource } from '../../core/embedded/HostAccent.js';
import { accentThemeCss } from './AccentThemeStyles.js';

/** Persist the user's accent independently of every workflow document. */
export const ACCENT_COLOR_SETTING = 'SugarCubes.Appearance.AccentColor';
/** Persist the accent source chosen under a system-accent host, apart from the custom color. */
export const ACCENT_SOURCE_SETTING = 'SugarCubes.Appearance.AccentSource';

/**
 * Project one accepted accent into mounted and future surfaces without rebuilding graph inventories.
 *
 * Precedence, highest first: a host presentation override (hosts using the host preference
 * policy), the --sugarcubes-accent root CSS property, then the selected accent source.
 * A host that declares a system accent defaults to it when available and offers a source choice.
 * Custom mode and other hosts use the saved color or {@link DEFAULT_ACCENT_COLOR}; the custom
 * color survives source changes.
 */
export class AccentThemeHost {
  readonly #listeners = new Set<() => void>();
  #theme: Readonly<{ color: string; dark: boolean }> = Object.freeze({
    color: DEFAULT_ACCENT_COLOR,
    dark: true,
  });
  readonly #document: Document;
  readonly #app: unknown;
  readonly #report: (error: unknown) => void;
  readonly #style: HTMLStyleElement;
  readonly #observer: MutationObserver;
  readonly #styleTexts = new WeakMap<HTMLStyleElement, string>();
  /** The user's explicitly saved accent, or null when none is stored. */
  #configured: string | null = null;
  #source: 'system' | 'custom' = 'system';
  #sourceOffered = false;
  #releaseHost: (() => void) | null = null;
  /** Drawn accent rows, held weakly by their preview element so a closed dialog's rows can go. */
  readonly #pickers = new Set<WeakRef<HTMLElement>>();
  readonly #pickerControls = new WeakMap<HTMLElement, AccentColorControls>();
  #registering = false;
  #releaseHostAccent: (() => void) | null = null;
  #signature = '';
  #queued = false;
  #disposed = false;
  #setup = false;
  #releaseEmbedded: (() => void) | null = null;

  /** Expose the authoritative accepted palette independently of node renderer presentation. */
  get theme(): Readonly<{ color: string; dark: boolean }> {
    return this.#theme;
  }

  /** Observe palette changes without introducing another document observer or setting owner. */
  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** Bind the host-facing settings and CSS boundary without claiming external variables. */
  constructor(
    documentRef: Document,
    app: unknown,
    reportError: (error: unknown) => void,
    private readonly embedded?: EmbeddedPresentationSource,
    private readonly hostAccent?: HostAccentSource,
    private readonly host?: HostDescriptionSource,
  ) {
    this.#document = documentRef;
    this.#app = app;
    this.#report = reportError;
    this.#style = documentRef.createElement('style');
    this.#style.id = 'sugarcubes-accent-theme';
    this.#observer = new MutationObserver((records) => {
      if (this.#paletteChanged(records)) this.#schedule();
    });
  }

  /** Register the color picker and observe only host palette inputs and stylesheet lifecycles. */
  setup(): void {
    if (this.#setup || this.#disposed) return;
    this.#setup = true;
    const settings = this.#settings();
    // Before registration Comfy reports only a stored value; its default is not yet known.
    if (typeof settings.getSettingValue === 'function')
      this.#configured = normalizeAccentColor(
        settings.getSettingValue.call(settings, ACCENT_COLOR_SETTING),
      );
    // Comfy announces the registered value synchronously; that announcement is not a user choice.
    this.#registering = true;
    try {
      if (typeof settings.addSetting === 'function')
        settings.addSetting.call(settings, {
          id: ACCENT_COLOR_SETTING,
          name: 'Accent color',
          type: 'hidden',
          defaultValue: DEFAULT_ACCENT_COLOR,
          onChange: (value: unknown) => {
            if (this.#registering) return;
            const color = normalizeAccentColor(value);
            if (!color) {
              this.#report(
                new TypeError('SugarCubes accent must be a hexadecimal color such as #b58cff.'),
              );
              return;
            }
            this.#configured = color;
            this.refresh();
          },
        });
    } finally {
      this.#registering = false;
    }
    this.#contributeAccentRow();
    this.#document.head.append(this.#style);
    for (const style of this.#document.head.querySelectorAll('style'))
      this.#styleTexts.set(style, style.textContent);
    this.#document.head.addEventListener('load', this.#onStylesheetLoad, true);
    for (const root of [this.#document.documentElement, this.#document.body])
      this.#observer.observe(root, {
        attributes: true,
        attributeFilter: ['class', 'style', 'data-sc-visual-style', 'data-sc-photopro-palette'],
      });
    this.#observer.observe(this.#document.head, {
      childList: true,
      subtree: true,
      characterData: true,
      attributes: true,
      attributeFilter: ['href', 'media', 'disabled'],
    });
    this.#releaseEmbedded = this.embedded?.subscribe(() => this.refresh()) ?? null;
    this.#releaseHostAccent = this.hostAccent?.subscribe(() => this.refresh()) ?? null;
    this.#releaseHost = this.host?.subscribe(() => this.#offerSystemAccent()) ?? null;
    this.#offerSystemAccent();
    this.refresh();
  }

  /** Whether the connected host declares that its accent is the system accent. */
  #systemAccentDeclared(): boolean {
    return this.host?.read()?.declares(HOST_CAPABILITIES.systemAccent) === true;
  }

  /**
   * Offer the accent source choice once the host declares a system accent.
   *
   * The declaration can settle after setup, so the choice is registered when it first appears.
   */
  #offerSystemAccent(): void {
    if (this.#sourceOffered || this.#disposed || !this.#systemAccentDeclared()) return;
    this.#sourceOffered = true;
    const settings = this.#settings();
    if (typeof settings.getSettingValue === 'function')
      this.#source =
        settings.getSettingValue.call(settings, ACCENT_SOURCE_SETTING) === 'custom'
          ? 'custom'
          : 'system';
    this.#registering = true;
    try {
      if (typeof settings.addSetting === 'function')
        settings.addSetting.call(settings, {
          id: ACCENT_SOURCE_SETTING,
          name: 'Accent source',
          type: 'hidden',
          defaultValue: 'system',
          onChange: (value: unknown) => {
            if (this.#registering) return;
            if (value !== 'system' && value !== 'custom') {
              this.#report(new TypeError('The accent source must be system or custom.'));
              return;
            }
            this.#source = value;
            this.refresh();
          },
        });
    } finally {
      this.#registering = false;
    }
    this.#contributeAccentRow();
    this.refresh();
  }

  /**
   * The Appearance page's Accent color row, as in Python: the colour, and once the host declares
   * a system accent, the choice between it and a custom colour.
   */
  #contributeAccentRow(): void {
    const settings = this.#settings();
    const save = (color: string): void => {
      if (typeof settings.setSettingValue !== 'function') {
        this.#report(new Error('Comfy accent setting persistence is unavailable.'));
        return;
      }
      try {
        void Promise.resolve(
          settings.setSettingValue.call(settings, ACCENT_COLOR_SETTING, color),
        ).catch(this.#report);
      } catch (error) {
        this.#report(error);
      }
    };
    // The row draws its controls in order: the preview creates one row's controls, the actions
    // after the source choice take them.
    let drawing: AccentColorControls | null = null;
    const preview = {
      kind: 'element' as const,
      create: (document: Document) => {
        const controls = createAccentColorControls(
          document,
          this.#preferred(),
          save,
          !this.#sourceOffered,
        );
        controls.update(this.#preferred(), this.#usesSystem());
        this.#pickers.add(new WeakRef(controls.preview));
        this.#pickerControls.set(controls.preview, controls);
        drawing = controls;
        return controls.preview;
      },
    };
    const actions = {
      kind: 'element' as const,
      create: (document: Document) => {
        const controls = drawing;
        drawing = null;
        return controls?.actions ?? document.createElement('span');
      },
    };
    const source = this.#sourceOffered
      ? [
          {
            kind: 'choice' as const,
            value: new ComfySettingStore(this.#app).value(ACCENT_SOURCE_SETTING, (value) =>
              value === 'custom' ? 'custom' : 'system',
            ),
            choices: fixed([
              { value: 'custom', label: 'Custom' },
              { value: 'system', label: 'System' },
            ]),
          },
        ]
      : [];
    settingsCatalogFor(this.#app).contribute('appearance.system_colors.accent', {
      controls: [preview, ...source, actions],
    });
  }

  /** Use the host's system accent only in system mode, retaining the custom browser fallback. */
  #preferred(): string {
    return (
      (this.#usesSystem() ? normalizeAccentColor(this.hostAccent?.read()) : null) ??
      this.#configured ??
      DEFAULT_ACCENT_COLOR
    );
  }

  /** An unavailable system accent falls back to the editable custom color. */
  #usesSystem(): boolean {
    return (
      this.#systemAccentDeclared() &&
      this.#source === 'system' &&
      !!normalizeAccentColor(this.hostAccent?.read())
    );
  }

  /** Resolve the accent by the documented precedence and publish only an actual palette change. */
  refresh(): void {
    if (this.#disposed || !this.#setup) return;
    for (const reference of this.#pickers) {
      const preview = reference.deref();
      if (preview) this.#pickerControls.get(preview)?.update(this.#preferred(), this.#usesSystem());
      else this.#pickers.delete(reference);
    }
    const style = this.#document.defaultView?.getComputedStyle(this.#document.documentElement);
    const color =
      normalizeAccentColor(this.embedded?.read()?.accentColor) ??
      normalizeAccentColor(style?.getPropertyValue('--sugarcubes-accent')) ??
      this.#preferred();
    const dark = readOwnedDarkTheme(this.#document);
    const signature = `${color}:${dark}`;
    if (signature === this.#signature) return;
    this.#signature = signature;
    this.#style.textContent = accentThemeCss(color, dark);
    this.#theme = Object.freeze({ color, dark });
    for (const listener of this.#listeners) listener();
    const app = isRecord(this.#app) ? this.#app : {};
    const canvas = isRecord(app.canvas) ? app.canvas : {};
    if (typeof canvas.setDirty === 'function') canvas.setDirty.call(canvas, true, true);
  }

  /** Ignore identical stylesheet rewrites from native widget rendering before forcing computed palette style. */
  #paletteChanged(records: readonly MutationRecord[]): boolean {
    let changed = false;
    const styles = new Set<HTMLStyleElement>();
    for (const record of records) {
      if (record.target === this.#style || this.#style.contains(record.target)) continue;
      if (
        record.target === this.#document.documentElement ||
        record.target === this.#document.body
      ) {
        changed = true;
        continue;
      }
      const target = record.target instanceof Element ? record.target : record.target.parentElement;
      const style = target?.closest('style');
      if (style instanceof HTMLStyleElement) styles.add(style);
      if (record.type === 'attributes' && style instanceof HTMLStyleElement) changed = true;
      if (target instanceof HTMLLinkElement) changed = true;
      for (const node of record.addedNodes) {
        if (node instanceof HTMLStyleElement) styles.add(node);
        else if (node instanceof HTMLLinkElement) changed = true;
      }
      for (const node of record.removedNodes)
        if (node instanceof HTMLStyleElement || node instanceof HTMLLinkElement) changed = true;
    }
    for (const style of styles) {
      const text = style.textContent;
      if (this.#styleTexts.get(style) !== text) changed = true;
      this.#styleTexts.set(style, text);
    }
    return changed;
  }

  /** Release presentation overrides while preserving saved preferences and external host styles. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#listeners.clear();
    this.#pickers.clear();
    this.#releaseEmbedded?.();
    this.#releaseHostAccent?.();
    this.#releaseHost?.();
    this.#releaseHost = null;
    this.#observer.disconnect();
    this.#document.head.removeEventListener('load', this.#onStylesheetLoad, true);
    this.#style.remove();
  }

  /** Coalesce styles and palette events without observing native controls or graph topology. */
  #schedule(): void {
    if (this.#queued || this.#disposed) return;
    this.#queued = true;
    queueMicrotask(() => {
      this.#queued = false;
      try {
        this.refresh();
      } catch (error) {
        this.#report(error);
      }
    });
  }

  /** Resolve a newly loaded external stylesheet after its rules join the cascade. */
  readonly #onStylesheetLoad = (event: Event): void => {
    if (event.target instanceof HTMLLinkElement && event.target.rel === 'stylesheet')
      this.#schedule();
  };

  /** Narrow dynamic Comfy settings before accepting host operations. */
  #settings(): UnknownRecord {
    const app = isRecord(this.#app) ? this.#app : {};
    const ui = isRecord(app.ui) ? app.ui : {};
    return isRecord(ui.settings) ? ui.settings : {};
  }
}
