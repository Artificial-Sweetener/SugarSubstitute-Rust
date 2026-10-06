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
/** Present model-library configuration through SugarCubes' existing settings and dialogs. */
import { isRecord } from '../types/common.js';
import type { ModalService } from '../dialogs/ModalService.js';
import type { ModelCatalogClient } from './ModelCatalogClient.js';
import type { ModelRootClient } from './ModelRootClient.js';

export interface ModelLibrarySettingsOptions {
  readonly app: unknown;
  readonly document: Document;
  readonly roots: ModelRootClient;
  readonly catalog: ModelCatalogClient;
  readonly dialogs: Pick<ModalService, 'openForm' | 'alert'>;
  reportError(error: unknown): void;
}

/** Keep library actions out of the picker renderer and the workflow state owner. */
export class ModelLibrarySettings {
  readonly #options: ModelLibrarySettingsOptions;
  #registered = false;

  /** Bind public library clients and the established SugarCubes dialog service. */
  constructor(options: ModelLibrarySettingsOptions) {
    this.#options = options;
  }

  /** Register one owned settings action row without creating persisted browser paths. */
  setup(): void {
    if (this.#registered) return;
    const app = isRecord(this.#options.app) ? this.#options.app : {};
    const ui = isRecord(app.ui) ? app.ui : {};
    const settings = isRecord(ui.settings) ? ui.settings : {};
    if (typeof settings.addSetting !== 'function')
      throw new Error('Comfy model-library settings are unavailable.');
    this.#registered = true;
    let row: HTMLElement | null = null;
    settings.addSetting.call(settings, {
      id: 'SugarCubes.Models.Library',
      name: 'Model library',
      category: ['SugarCubes', 'Models', 'Library'],
      defaultValue: '',
      tooltip: 'Configure the shared model folder or refresh the prepared model catalog.',
      type: () => {
        if (row) return row;
        row = this.#options.document.createElement('div');
        row.className = 'sugarcubes-settings-row';
        Object.assign(row.style, { display: 'flex', gap: '8px', justifyContent: 'flex-end' });
        row.append(
          this.#button('Model folder…', () => this.#configure()),
          this.#button('Download organization…', () => this.#organization()),
          this.#button('Refresh catalog', () => this.#options.catalog.refresh()),
        );
        return row;
      },
    });
  }

  /** Configure optional folders beneath each model category through the shared modal system. */
  async #organization(): Promise<void> {
    const pattern = await this.#options.catalog.downloadPattern();
    const values = await this.#options.dialogs.openForm({
      title: 'Download organization',
      message: [
        'New downloads are saved beneath their model category folder.',
        'Use {architecture}, {family}, {nsfw} and {file_name}. Unknown metadata uses Unknown or unknown.',
        'Example: {architecture}/{family}/{nsfw}/{file_name}',
      ],
      fields: [
        {
          key: 'pattern',
          label: 'Folder pattern',
          initialValue: pattern,
          normalizeValue: (value) => value.trim(),
        },
      ],
      confirmLabel: 'Save',
    });
    if (values && typeof values.pattern === 'string')
      await this.#options.catalog.saveDownloadPattern(values.pattern);
  }

  /** Own one button's pending state and route failures through existing SugarCubes feedback. */
  #button(label: string, operation: () => Promise<void>): HTMLButtonElement {
    const button = this.#options.document.createElement('button');
    button.type = 'button';
    button.className = 'p-button p-button-secondary';
    button.textContent = label;
    button.addEventListener('click', () => {
      button.disabled = true;
      void operation()
        .catch(this.#options.reportError)
        .finally(() => {
          button.disabled = false;
        });
    });
    return button;
  }

  /** Edit next-launch intent without relocating files or restarting the current Comfy host. */
  async #configure(): Promise<void> {
    const status = await this.#options.roots.load();
    const values = await this.#options.dialogs.openForm({
      title: 'Model folder',
      message: [
        `Active folder: ${status.activeModelRoot}`,
        'Changes apply after ComfyUI restarts. Existing model files stay in their current folders.',
      ],
      fields: [
        {
          key: 'useDefault',
          type: 'checkbox',
          label: 'Use ComfyUI’s default model folder',
          initialValue: status.usesDefault,
        },
        {
          key: 'path',
          label: 'Model folder',
          initialValue: status.configuredModelRoot ?? status.activeModelRoot,
          normalizeValue: (value) => value.trim(),
          validate: (value, all) =>
            all.useDefault === true || (typeof value === 'string' && value.length > 0)
              ? null
              : 'Enter an absolute model folder path.',
        },
      ],
      confirmLabel: 'Save model folder',
    });
    if (!values) return;
    const path =
      values.useDefault === true ? null : typeof values.path === 'string' ? values.path : '';
    const saved = await this.#options.roots.configure(path);
    await this.#options.dialogs.alert({
      title: 'Model folder saved',
      message: saved.restartRequired
        ? 'Restart ComfyUI to use the selected model folder.'
        : 'ComfyUI is already using the selected model folder.',
    });
  }
}
