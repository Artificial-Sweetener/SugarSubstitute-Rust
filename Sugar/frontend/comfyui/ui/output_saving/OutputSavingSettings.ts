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
/**
 * The Generation page's Output files rows, as Python presents them: output folder, output
 * pattern, the example path, saved cube outputs, JPEG companions with their sizing, Drag and Copy
 * format, workflow saving and its Preview Image outputs, each open workflow's outputs, and the
 * settings import. Every change is saved at once; the server keeps the settings and renders the
 * example path.
 */
import { settingsCatalogFor } from '../settings/workspace/SettingsCatalog.js';
import {
  fixed,
  SettingsState,
  type SettingsFact,
  type SettingsValue,
} from '../settings/workspace/SettingsModel.js';
import type { OutputSavingCommands } from './OutputSavingConfiguration.js';
import type { OutputSavingPreferences, OutputSavingSnapshot } from './OutputSavingPreferences.js';
import { pickSubstituteSettings } from './SubstituteSettingsPicker.js';

/** Keep the server authoritative; one change is saved at a time. */
export class OutputSavingSettings {
  readonly #snapshot = new SettingsState<OutputSavingSnapshot | null>(null);
  readonly #busy = new SettingsState(false);
  #registered = false;

  constructor(
    private readonly document: Document,
    private readonly client: OutputSavingCommands,
    private readonly reportError: (error: unknown) => void,
    private readonly manageSources: (() => Promise<void>) | null,
  ) {}

  /** Contribute the rows and read the settings. */
  register(app: unknown): void {
    if (this.#registered) return;
    this.#registered = true;
    const catalog = settingsCatalogFor(app);
    const ready = this.#fact((value) => value !== null && !this.#busy.read());
    const jpegOn = this.#fact(
      (value) => value?.preferences.jpeg.enabled === true && !this.#busy.read(),
    );
    catalog.contribute('generation.output.folder', {
      controls: [
        {
          kind: 'text',
          value: this.#field(
            (p) => p.organization.output_root ?? '',
            (p, next) => ({
              ...p,
              organization: { ...p.organization, output_root: next.trim() || null },
            }),
            '',
          ),
          width: 420,
          placeholder: 'ComfyUI’s output folder',
        },
        {
          kind: 'button',
          button: {
            label: 'Default',
            run: () =>
              this.#save((p) => ({ ...p, organization: { ...p.organization, output_root: null } })),
          },
        },
      ],
      enabled: ready,
    });
    catalog.contribute('generation.output.pattern', {
      controls: [
        {
          kind: 'text',
          value: this.#field(
            (p) => p.organization.path_pattern,
            (p, next) => ({ ...p, organization: { ...p.organization, path_pattern: next } }),
            '',
          ),
          width: 360,
        },
      ],
      enabled: ready,
    });
    catalog.contribute('generation.output.preview', {
      controls: [
        {
          kind: 'text',
          value: { ...this.#fact((value) => value?.examplePath ?? ''), write: () => undefined },
          width: 420,
          readOnly: true,
        },
      ],
    });
    catalog.contribute('generation.output.persistence', {
      controls: [
        {
          kind: 'choice',
          value: this.#field(
            (p) => p.persistence_mode,
            (p, next) => ({ ...p, persistence_mode: next === 'final_cube' ? 'final_cube' : 'all' }),
            'all',
          ),
          choices: fixed([
            { value: 'all', label: 'Every cube' },
            { value: 'final_cube', label: 'Final active cube' },
          ]),
        },
      ],
      enabled: ready,
    });
    const jpeg = this.#field(
      (p) => p.jpeg.enabled,
      (p, next) => ({ ...p, jpeg: { ...p.jpeg, enabled: next } }),
      false,
    );
    const sizing = this.#field(
      (p) => p.jpeg.sizing_mode,
      (p, next) => ({
        ...p,
        jpeg: { ...p.jpeg, sizing_mode: next === 'target_size' ? 'target_size' : 'quality' },
      }),
      'quality',
    );
    catalog.contribute('generation.output.jpeg', {
      expander: {
        switch: jpeg,
        rows: [
          {
            id: 'generation.output.jpeg.sizing',
            title: 'JPEG sizing',
            description: 'Size each companion by a fixed quality or an approximate file size.',
            controls: [
              {
                kind: 'choice',
                value: sizing,
                choices: fixed([
                  { value: 'quality', label: 'Fixed quality' },
                  { value: 'target_size', label: 'Approximate target size' },
                ]),
                width: 200,
              },
            ],
            enabled: jpegOn,
          },
          {
            id: 'generation.output.jpeg.quality',
            title: 'JPEG quality',
            description: 'Also the quality of JPEG copies for drag and Copy.',
            controls: [
              {
                kind: 'slider',
                value: this.#field(
                  (p) => p.jpeg.quality,
                  (p, next) => ({ ...p, jpeg: { ...p.jpeg, quality: Math.round(next) } }),
                  90,
                ),
                min: 1,
                max: 100,
                step: 1,
              },
            ],
            enabled: ready,
          },
          {
            id: 'generation.output.jpeg.target',
            title: 'Target size (KiB)',
            description: 'The approximate size of each companion.',
            controls: [
              {
                kind: 'number',
                value: this.#field(
                  (p) => p.jpeg.target_size_kib,
                  (p, next) => ({
                    ...p,
                    jpeg: { ...p.jpeg, target_size_kib: Math.max(1, Math.round(next)) },
                  }),
                  512,
                ),
                min: 1,
                step: 1,
              },
            ],
            visible: {
              read: () => sizing.read() === 'target_size',
              subscribe: (l) => sizing.subscribe(l),
            },
            enabled: jpegOn,
          },
        ],
      },
      enabled: ready,
    });
    catalog.contribute('generation.output.transfer', {
      controls: [
        {
          kind: 'switch',
          value: this.#field(
            (p) => p.transfer.preferred_format === 'companion_jpeg',
            (p, next) => ({
              ...p,
              transfer: { preferred_format: next ? 'companion_jpeg' : 'canonical_png' },
            }),
            false,
          ),
        },
      ],
      enabled: jpegOn,
    });
    const managed = this.#field(
      (p) => p.manage_workflow,
      (p, next) => ({ ...p, manage_workflow: next }),
      false,
    );
    catalog.contribute('generation.output.workflow', {
      controls: [{ kind: 'switch', value: managed }],
      enabled: ready,
    });
    catalog.contribute('generation.output.previews', {
      controls: [
        {
          kind: 'switch',
          value: this.#field(
            (p) => p.save_previews,
            (p, next) => ({ ...p, save_previews: next }),
            false,
          ),
        },
      ],
      enabled: this.#fact(
        (value) => value?.preferences.manage_workflow === true && !this.#busy.read(),
      ),
    });
    if (this.manageSources) {
      const manageSources = this.manageSources;
      catalog.contribute('generation.output.sources', {
        controls: [{ kind: 'button', button: { label: 'Workflow outputs…', run: manageSources } }],
      });
    }
    catalog.contribute('generation.output.import', {
      controls: [
        {
          kind: 'button',
          button: {
            label: 'Import…',
            run: async () => {
              const value = await pickSubstituteSettings(this.document);
              if (value !== null) this.#snapshot.set(await this.client.importSubstitute(value));
            },
          },
        },
      ],
      enabled: ready,
    });
    void this.refresh();
  }

  /** Read the settings again, as when another window changed them. */
  refresh(): Promise<void> {
    if (this.#busy.read()) return Promise.resolve();
    return this.client
      .load()
      .then((value) => this.#snapshot.set(value))
      .catch(this.reportError);
  }

  /** A fact read from the current settings, following saves and reloads. */
  #fact<T>(read: (value: OutputSavingSnapshot | null) => T): SettingsFact<T> {
    return {
      read: () => read(this.#snapshot.read()),
      subscribe: (listener) => {
        const stopSnapshot = this.#snapshot.subscribe(listener);
        const stopBusy = this.#busy.subscribe(listener);
        return () => {
          stopSnapshot();
          stopBusy();
        };
      },
    };
  }

  /** One field of the settings, saved as a whole on change. */
  #field<T>(
    read: (preferences: OutputSavingPreferences) => T,
    apply: (preferences: OutputSavingPreferences, next: T) => OutputSavingPreferences,
    fallback: T,
  ): SettingsValue<T> {
    return {
      read: () => {
        const value = this.#snapshot.read();
        return value ? read(value.preferences) : fallback;
      },
      write: (next) => this.#save((preferences) => apply(preferences, next)),
      subscribe: this.#fact(() => null).subscribe,
    };
  }

  async #save(
    change: (preferences: OutputSavingPreferences) => OutputSavingPreferences,
  ): Promise<void> {
    const current = this.#snapshot.read();
    if (!current || this.#busy.read()) return;
    this.#busy.set(true);
    try {
      this.#snapshot.set(await this.client.save(change(current.preferences)));
    } finally {
      this.#busy.set(false);
    }
  }
}
