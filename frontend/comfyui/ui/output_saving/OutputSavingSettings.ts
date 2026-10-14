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
/** Present configured output saving and explicit settings import in Comfy's SugarCubes settings. */
import type { SettingsManager } from '../settings/HostSettingsController.js';
import type { ModalAdapter } from '../dialogs/ModalShell.js';
import { OutputSavingDialog } from './OutputSavingDialog.js';
import type { OutputSavingCommands } from './OutputSavingConfiguration.js';
import type { OutputSavingSnapshot } from './OutputSavingPreferences.js';
import { pickSubstituteSettings } from './SubstituteSettingsPicker.js';

/** Keep the server authoritative while retaining one row's control identity and explicit pending state. */
export class OutputSavingSettings {
  readonly #dialog: OutputSavingDialog;
  #manager: SettingsManager | null = null;
  #row: HTMLElement | null = null;
  #summary: HTMLElement | null = null;
  #buttons: HTMLButtonElement[] = [];
  #value: OutputSavingSnapshot | null = null;
  #loading: Promise<void> | null = null;
  #busy = false;
  #revision = 0;

  /** Bind existing modal, transport and feedback owners without introducing canvas policy. */
  constructor(
    private readonly document: Document,
    private readonly client: OutputSavingCommands,
    adapter: ModalAdapter,
    private readonly reportError: (error: unknown) => void,
    private readonly manageSources: (() => Promise<void>) | null,
  ) {
    this.#dialog = new OutputSavingDialog(document, client, adapter);
  }

  /** Register an action row whose controls read shared settings each time the panel is entered. */
  register(manager: SettingsManager): void {
    if (this.#manager === manager) return;
    this.#manager = manager;
    manager.addSetting({
      id: 'SugarCubes.Output.Files',
      name: 'Output files',
      category: ['SugarCubes', 'Output', 'Files'],
      defaultValue: '',
      tooltip:
        'Configure output folder, filename pattern, saved cubes, JPEG companions, workflow management, and Drag and Copy format.',
      type: () => {
        if (!this.#row) {
          this.#row = this.document.createElement('div');
          this.#row.className = 'sugarcubes-settings-row';
          this.#row.style.cssText = 'display:flex;align-items:center;gap:8px;flex-wrap:wrap';
          this.#summary = this.document.createElement('span');
          this.#summary.style.flex = '1';
          this.#row.append(
            this.#summary,
            this.#button('Configure…', async () => {
              const snapshot = await this.client.load();
              const saved = await this.#dialog.open(snapshot);
              this.#value = saved ?? snapshot;
            }),
            this.#button('Import Substitute settings…', async () => {
              const value = await pickSubstituteSettings(this.document);
              if (value !== null) this.#value = await this.client.importSubstitute(value);
            }),
            ...(this.manageSources ? [this.#button('Workflow outputs…', this.manageSources)] : []),
          );
        }
        this.show();
        void this.refresh();
        return this.#row;
      },
    });
  }

  /** Re-read shared settings once without reconstructing the controls or fabricating fallback defaults. */
  refresh(): Promise<void> {
    if (this.#busy) return Promise.resolve();
    if (this.#loading) return this.#loading;
    const revision = this.#revision;
    this.#loading = this.client
      .load()
      .then((value) => {
        if (revision !== this.#revision) return;
        this.#value = value;
        this.show();
      })
      .catch(this.reportError)
      .finally(() => {
        this.#loading = null;
      });
    return this.#loading;
  }

  /** Describe the two independent scopes and reconcile control state after accepted or failed operations. */
  show(): void {
    for (const button of this.#buttons) button.disabled = this.#busy;
    if (!this.#summary) return;
    const value = this.#value?.preferences;
    this.#summary.textContent = value
      ? (value.persistence_mode === 'all' ? 'Every cube' : 'Final active cube') +
        ' · Workflow management ' +
        (value.manage_workflow ? 'on' : 'off')
      : 'Output settings';
  }

  /** Route asynchronous actions through existing feedback while preserving picker and modal user activation. */
  #button(label: string, operation: () => Promise<void>): HTMLButtonElement {
    const button = this.document.createElement('button');
    button.type = 'button';
    button.className = 'p-button p-button-secondary';
    button.textContent = label;
    this.#buttons.push(button);
    button.addEventListener('click', () => {
      if (this.#busy) return;
      this.#revision++;
      this.#busy = true;
      this.show();
      void operation()
        .catch(this.reportError)
        .finally(() => {
          this.#busy = false;
          this.show();
        });
    });
    return button;
  }
}
