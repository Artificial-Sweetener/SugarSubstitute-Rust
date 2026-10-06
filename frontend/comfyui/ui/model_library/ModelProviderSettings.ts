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
/** Present reference CivitAI settings through the shared SugarCubes preference owner. */
import { isRecord } from '../types/common.js';
import {
  createFluentBooleanCheckbox,
  fluentTokenDeclarations,
} from '../surface/appearance/FluentThemeRuntime.js';
import { readComfyDarkTheme } from '../surface/appearance/ComfyColorScheme.js';
import type { FluentBooleanCheckbox } from '../surface/appearance/NativeToggleRows.js';
import {
  modelThumbnailSafetyChoices,
  type ModelProviderPreferences,
  type ModelProviderPreferenceClient,
} from './ModelProviderPreferences.js';

/** Render provider policy without manufacturing defaults or persisting browser-local choices. */
export class ModelProviderSettings {
  readonly #document: Document;
  readonly #client: ModelProviderPreferenceClient;
  readonly #report: (error: unknown) => void;
  readonly #createCheckbox: (document: Document) => FluentBooleanCheckbox;
  readonly #render: (() => void)[] = [];
  #value: ModelProviderPreferences | null = null;
  #loading: Promise<void> | null = null;
  #busy = false;
  #registered = false;

  /** Bind shared settings transport and the established non-blocking feedback owner. */
  constructor(
    document: Document,
    client: ModelProviderPreferenceClient,
    report: (error: unknown) => void,
    createCheckbox: (document: Document) => FluentBooleanCheckbox = createFluentBooleanCheckbox,
  ) {
    this.#document = document;
    this.#client = client;
    this.#report = report;
    this.#createCheckbox = createCheckbox;
  }

  /** Register separate reference controls while keeping server policy authoritative. */
  setup(app: unknown): void {
    if (this.#registered) return;
    const host = isRecord(app) ? app : {};
    const ui = isRecord(host.ui) ? host.ui : {};
    const settings = isRecord(ui.settings) ? ui.settings : {};
    if (typeof settings.addSetting !== 'function')
      throw new Error('Comfy provider settings are unavailable.');
    this.#registered = true;
    for (const [key, name, tooltip] of [
      [
        'metadataLookupEnabled',
        'Look up local model metadata',
        'Use CivitAI hashes to identify local models.',
      ],
      [
        'missingModelLookupEnabled',
        'Look up missing models',
        'Use CivitAI hashes to resolve missing model references.',
      ],
      [
        'downloadsEnabled',
        'Download missing CivitAI models',
        'Allow missing-model resolution to offer CivitAI downloads.',
      ],
      [
        'thumbnailDownloadsEnabled',
        'Download model thumbnails',
        'Download CivitAI preview images for model fields and pickers.',
      ],
      [
        'thumbnailSafety',
        'Thumbnail safety',
        'Choose which CivitAI images may be used as model thumbnails.',
      ],
    ] as const) {
      const booleanKey = key === 'thumbnailSafety' ? null : key;
      let row: HTMLElement | null = null;
      settings.addSetting.call(settings, {
        id: 'SugarCubes.Models.' + key,
        name,
        category: ['SugarCubes', 'Models', key],
        defaultValue: '',
        tooltip,
        type: () => {
          if (row) {
            this.#load();
            return row;
          }
          row = this.#document.createElement('div');
          row.className = 'sugarcubes-settings-row';
          let control: HTMLSelectElement | FluentBooleanCheckbox;
          if (key === 'thumbnailSafety') {
            const select = this.#document.createElement('select');
            select.className = 'p-inputtext p-component';
            for (const choice of modelThumbnailSafetyChoices) {
              const option = this.#document.createElement('option');
              option.value = choice.value;
              option.textContent = choice.label;
              select.append(option);
            }
            control = select;
          } else {
            control = this.#createCheckbox(this.#document);
            const caption = this.#document.createElement('span');
            caption.textContent = 'Enabled';
            const group = this.#document.createElement('span');
            group.style.cssText =
              'display:inline-flex;align-items:center;gap:8px;white-space:nowrap';
            // Microsoft's checkbox has indicator slots only, so its caption must be a sibling.
            group.append(control, caption);
            const checkbox = control;
            caption.addEventListener('click', () => {
              if (!checkbox.disabled) checkbox.click();
            });
            row.append(group);
          }
          control.setAttribute('aria-label', name);
          if (control instanceof HTMLSelectElement) row.append(control);
          this.#render.push(() => {
            if (row)
              row.style.cssText = fluentTokenDeclarations(readComfyDarkTheme(this.#document));
            control.disabled =
              this.#busy ||
              this.#value === null ||
              (key === 'thumbnailSafety' && !this.#value.thumbnailDownloadsEnabled);
            if (!this.#value) return;
            if (control instanceof HTMLSelectElement) control.value = this.#value.thumbnailSafety;
            else if (booleanKey) control.checked = this.#value[booleanKey];
          });
          control.addEventListener('change', () => {
            if (!this.#value || this.#busy) return;
            const patch: Partial<ModelProviderPreferences> =
              control instanceof HTMLSelectElement
                ? {
                    thumbnailSafety:
                      modelThumbnailSafetyChoices.find((choice) => choice.value === control.value)
                        ?.value ?? this.#value.thumbnailSafety,
                  }
                : booleanKey
                  ? { [booleanKey]: control.checked }
                  : {};
            this.#busy = true;
            this.#show();
            void this.#client
              .configure(patch)
              .then((value) => {
                this.#value = value;
              })
              .catch(this.#report)
              .finally(() => {
                this.#busy = false;
                this.#show();
              });
          });
          this.#show();
          this.#load();
          return row;
        },
      });
    }
  }

  /** Reconcile every mounted control after one shared response or failed edit. */
  #show(): void {
    for (const render of this.#render) render();
  }

  /** Re-read shared policy on settings entry and permit retry after unavailable transport. */
  #load(): void {
    if (this.#busy || this.#loading) return;
    this.#loading = this.#client
      .load()
      .then((value) => {
        this.#value = value;
        this.#show();
      })
      .catch(this.#report)
      .finally(() => {
        this.#loading = null;
      });
  }
}
