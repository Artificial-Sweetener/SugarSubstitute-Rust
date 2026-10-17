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
/** Follow the host's Fluent preference and theme without replacing live picker state. */
import type { VisualStyle } from '../surface/appearance/VisualStyle.js';
import { isRecord } from '../types/common.js';
import {
  VISUAL_STYLE_SETTING,
  readVisualStylePreference,
} from '../surface/appearance/VisualStylePreference.js';
import { readComfyDarkTheme } from '../surface/appearance/ComfyColorScheme.js';
import { applyOwnedTheme, visualStyleTokens } from '../surface/appearance/ThemeService.js';
import { modelPickerCss } from './ModelPickerStyles.js';

/** Own scoped theme annotations, styles and subscriptions for mounted picker surfaces. */
export class ModelPickerAppearance {
  readonly #document: Document;
  readonly #settings;
  readonly #style: HTMLStyleElement;
  readonly #observer: MutationObserver;
  readonly #surfaces = new Set<HTMLElement>();
  #active = false;
  #dark = true;

  /** Bind appearance to the same persisted preference as native Fluent cards. */
  constructor(
    documentRef: Document,
    private readonly app: unknown,
  ) {
    this.#document = documentRef;
    const ui = isRecord(app) && isRecord(app.ui) ? app.ui : {};
    this.#settings = isRecord(ui.settings) ? ui.settings : {};
    this.#style = documentRef.createElement('style');
    this.#style.id = 'sugarcubes-model-picker';
    this.#observer = new MutationObserver(this.#refresh);
    for (const root of [documentRef.documentElement, documentRef.body])
      this.#observer.observe(root, { attributes: true, attributeFilter: ['class', 'style'] });
    if (typeof this.#settings.addEventListener === 'function')
      this.#settings.addEventListener.call(
        this.#settings,
        `${VISUAL_STYLE_SETTING}.change`,
        this.#onPreferenceChange,
      );
  }

  /** Enable styling for the supported renderer lifetime. */
  activate(): void {
    this.#active = true;
    this.#document.head.append(this.#style);
    this.#refresh();
  }

  /** Theme one new field or portalled popup using the current host preference. */
  track(surface: HTMLElement): void {
    this.#surfaces.add(surface);
    this.#apply(surface);
  }

  /** Release scoped tokens before removing an owned surface. */
  release(surface: HTMLElement): void {
    this.#surfaces.delete(surface);
    applyOwnedTheme(surface, null);
    delete surface.dataset.scModelAppearance;
  }

  /** Restore host appearance when the supported renderer lifetime ends. */
  deactivate(): void {
    this.#active = false;
    this.#style.remove();
    for (const surface of [...this.#surfaces]) this.release(surface);
  }

  /** Remove preference observers after releasing all scoped presentation. */
  dispose(): void {
    this.deactivate();
    this.#observer.disconnect();
    if (typeof this.#settings.removeEventListener === 'function')
      this.#settings.removeEventListener.call(
        this.#settings,
        `${VISUAL_STYLE_SETTING}.change`,
        this.#onPreferenceChange,
      );
  }

  /** Read only the authoritative host preference, leaving control enablement independent. */
  #visualStyle(): VisualStyle {
    return readVisualStylePreference(this.app);
  }

  /** Read after Comfy commits the value following its synchronous change dispatch. */
  readonly #onPreferenceChange = (): void => {
    queueMicrotask(this.#refresh);
  };

  /** Refresh live surfaces in place so search, focus and keyboard selection survive theme changes. */
  readonly #refresh = (): void => {
    if (!this.#active) return;
    this.#dark = readComfyDarkTheme(this.#document);
    const style = this.#visualStyle();
    this.#style.textContent = `${modelPickerCss}\n[data-sc-model-appearance="${style}"]{${style === 'comfy' ? '' : visualStyleTokens(style, this.#dark)}}\n[data-sc-model-native]{display:none!important}.sc-model-mount{width:100%;min-width:0}`;
    for (const surface of this.#surfaces) this.#apply(surface);
  };

  /** Apply Microsoft tokens only to Fluent surfaces; native controls inherit Comfy tokens. */
  #apply(surface: HTMLElement): void {
    const style = this.#visualStyle();
    surface.dataset.scModelAppearance = style === 'comfy' ? 'native' : style;
    applyOwnedTheme(
      surface,
      style === 'comfy' ? null : this.#dark,
      style === 'comfy' ? 'fluent' : style,
    );
  }
}
