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
/** Keep the prompt editor's preferences in ComfyUI's settings and publish every accepted change. */
import { ComfySettingStore } from '../settings/workspace/ComfySettingStore.js';
import type { SettingsValue } from '../settings/workspace/SettingsModel.js';
import { PreferenceSession, defaultPreferences } from '../../../sugarprompt/services/src/index.js';
import type {
  FeatureId,
  FeaturePreferences,
  PreferenceAdapter,
  PreferencePatch,
} from '../../../sugarprompt/services/src/index.js';

/** The one ComfyUI setting that holds the editor's whole preference snapshot. */
export const PROMPT_EDITOR_PREFERENCES_SETTING = 'SugarCubes.PromptEditor.Preferences';

/** What prompt editors and their settings rows read: the accepted snapshot and its changes. */
export interface PromptEditorPreferenceSource {
  current(): FeaturePreferences;
  subscribe(listener: (preferences: FeaturePreferences) => void): () => void;
}

/** The defaults, for an editor whose host keeps no preferences. */
export const defaultPreferenceSource: PromptEditorPreferenceSource = Object.freeze({
  current: () => defaultPreferences,
  subscribe: () => () => undefined,
});

/**
 * The page's prompt editor preferences. ComfyUI persists the snapshot; the services'
 * PreferenceSession validates it, fills features added since it was saved from their defaults and
 * serializes edits so two switches changed together cannot lose each other.
 */
export class PromptEditorPreferences implements PromptEditorPreferenceSource {
  readonly #store: ComfySettingStore;
  readonly #session: PreferenceSession;
  readonly #listeners = new Set<(preferences: FeaturePreferences) => void>();
  readonly #report: (error: unknown) => void;
  #current: FeaturePreferences = defaultPreferences;
  #started = false;

  /** Bind to the page's ComfyUI app; nothing is read until `setup`. */
  constructor(app: unknown, report: (error: unknown) => void) {
    this.#report = report;
    this.#store = new ComfySettingStore(app);
    this.#session = new PreferenceSession(comfyPreferenceAdapter(this.#store), (preferences) => {
      if (preferences === this.#current) return;
      this.#current = preferences;
      for (const listener of [...this.#listeners]) listener(preferences);
    });
  }

  /** Register the hidden setting once ComfyUI's settings exist, and load what it holds. */
  setup(): void {
    if (this.#started) return;
    this.#started = true;
    this.#store.register({
      id: PROMPT_EDITOR_PREFERENCES_SETTING,
      name: 'Prompt editor preferences',
      type: 'hidden',
      defaultValue: null,
    });
    this.#session.load().then((outcome) => {
      if (outcome.status === 'invalid')
        this.#report(
          new TypeError('Stored prompt editor preferences are invalid; defaults are in use.'),
        );
    }, this.#report);
  }

  current(): FeaturePreferences {
    return this.#current;
  }

  subscribe(listener: (preferences: FeaturePreferences) => void): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  /** A Settings switch bound to whether one feature is allowed. */
  feature(id: FeatureId): SettingsValue<boolean> {
    return this.#value(
      () => this.#current.allowed[id],
      (allowed) => ({ allowed: { [id]: allowed } }),
    );
  }

  /** A Settings switch that is on while the wheel adjusts a hovered control. */
  wheelHover(): SettingsValue<boolean> {
    return this.#value(
      () => this.#current.wheelMode === 'hover_dwell',
      (hover) => ({ wheelMode: hover ? 'hover_dwell' : 'focus_required' }),
    );
  }

  /** A Settings switch bound to whether queued prompts have their wildcards resolved. */
  resolveOnGeneration(): SettingsValue<boolean> {
    return this.#value(
      () => this.#current.resolveOnGeneration,
      (resolve) => ({ resolveOnGeneration: resolve }),
    );
  }

  /** Stop publishing; edits still queued settle as disposed. */
  dispose(): void {
    this.#session.dispose();
    this.#listeners.clear();
  }

  #value(read: () => boolean, patch: (value: boolean) => PreferencePatch): SettingsValue<boolean> {
    return {
      read,
      write: async (value) => {
        const outcome = await this.#session.update(patch(value));
        if (outcome.status === 'invalid' || outcome.status === 'unavailable')
          this.#report(new Error('Prompt editor preferences could not be saved.'));
      },
      subscribe: (listener) => this.subscribe(() => listener()),
    };
  }
}

/** Read and write the snapshot through ComfyUI's setting store. */
function comfyPreferenceAdapter(store: ComfySettingStore): PreferenceAdapter {
  return {
    read: () =>
      Promise.resolve({
        status: 'ready',
        value: store.get(PROMPT_EDITOR_PREFERENCES_SETTING) ?? null,
      }),
    write: async (value) => {
      await store.set(PROMPT_EDITOR_PREFERENCES_SETTING, value);
      return { status: 'saved' };
    },
  };
}
