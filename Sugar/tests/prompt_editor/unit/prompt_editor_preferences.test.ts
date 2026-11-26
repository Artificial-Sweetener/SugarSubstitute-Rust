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
/** Prove Prompt Editing's switches keep the editor's preferences in ComfyUI's settings. */
import { jest } from '@jest/globals';
import {
  PROMPT_EDITOR_PREFERENCES_SETTING,
  PromptEditorPreferences,
} from '../../../frontend/comfyui/ui/prompt_editor/PromptEditorPreferences.js';
import { registerPromptEditingSettings } from '../../../frontend/comfyui/ui/prompt_editor/PromptEditingSettings.js';
import type { FeaturePreferences } from '../../../frontend/sugarprompt/services/src/index.js';
import { drawRows, settle } from '../../settings/support/SettingsRowHarness.js';

afterEach(() => document.body.replaceChildren());

/** A ComfyUI app whose settings store holds `stored` for the preference setting. */
function comfyApp(stored: unknown) {
  const values = new Map<string, unknown>([[PROMPT_EDITOR_PREFERENCES_SETTING, stored]]);
  const registered: Record<string, unknown>[] = [];
  const settings = Object.assign(new EventTarget(), {
    addSetting: (definition: Record<string, unknown>) => registered.push(definition),
  });
  const app = {
    ui: { settings },
    extensionManager: {
      setting: {
        settings: {},
        get: (id: string) => values.get(id),
        set: async (id: string, value: unknown) => {
          values.set(id, value);
        },
      },
    },
  };
  return { app, values, registered };
}

const ROWS = [
  'prompt_editing.interaction.wheel_hover',
  'prompt_editing.wildcards.resolve',
  'prompt_editing.features.emphasis',
  'prompt_editing.features.spellcheck',
  'prompt_editing.danbooru.danbooru_url_import',
];

test('first use reads every feature on, spellcheck included, and writes nothing', async () => {
  const comfy = comfyApp(undefined);
  const preferences = new PromptEditorPreferences(comfy.app, jest.fn());
  preferences.setup();
  await settle();
  const current = preferences.current();
  expect(Object.values(current.allowed).every(Boolean)).toBe(true);
  expect(current.allowed.spellcheck).toBe(true);
  expect(current.allowed.danbooru_url_import).toBe(true);
  expect(current.wheelMode).toBe('hover_dwell');
  expect(current.resolveOnGeneration).toBe(true);
  expect(comfy.registered).toEqual([
    expect.objectContaining({ id: PROMPT_EDITOR_PREFERENCES_SETTING, type: 'hidden' }),
  ]);
  expect(comfy.values.get(PROMPT_EDITOR_PREFERENCES_SETTING)).toBeUndefined();
});

test('the switches save to ComfyUI and every follower hears each change', async () => {
  const comfy = comfyApp(null);
  const report = jest.fn<(error: unknown) => void>();
  const preferences = new PromptEditorPreferences(comfy.app, report);
  preferences.setup();
  registerPromptEditingSettings(comfy.app, preferences);
  await settle();
  const heard: FeaturePreferences[] = [];
  preferences.subscribe((changed) => heard.push(changed));
  const rows = drawRows(comfy.app, ROWS, report);
  const toggle = async (id: string): Promise<void> => {
    rows.control<HTMLElement>(id, '[role="switch"], input[type="checkbox"]').click();
    for (let turn = 0; turn < 5; turn++) await settle();
  };

  await toggle('prompt_editing.features.emphasis');
  await toggle('prompt_editing.interaction.wheel_hover');
  await toggle('prompt_editing.wildcards.resolve');
  await toggle('prompt_editing.danbooru.danbooru_url_import');

  const saved = comfy.values.get(PROMPT_EDITOR_PREFERENCES_SETTING) as FeaturePreferences;
  expect(saved.allowed.emphasis).toBe(false);
  expect(saved.allowed.danbooru_url_import).toBe(false);
  expect(saved.allowed.spellcheck).toBe(true);
  expect(saved.wheelMode).toBe('focus_required');
  expect(saved.resolveOnGeneration).toBe(false);
  expect(heard).toHaveLength(4);
  expect(heard.at(-1)).toEqual(saved);
  expect(report).not.toHaveBeenCalled();

  // A later page reads what was saved, with features added since filled from their defaults.
  const { duplicate_segment_diagnostics: _added, ...olderAllowed } = saved.allowed;
  const later = comfyApp({ ...saved, allowed: olderAllowed });
  const reloaded = new PromptEditorPreferences(later.app, report);
  reloaded.setup();
  await settle();
  expect(reloaded.current().allowed.emphasis).toBe(false);
  expect(reloaded.current().allowed.duplicate_segment_diagnostics).toBe(true);
  expect(reloaded.current().wheelMode).toBe('focus_required');
  rows.dispose();
});

test('a stored value that is not a preference snapshot keeps the defaults and is reported', async () => {
  const comfy = comfyApp({ allowed: { emphasis: 'yes' } });
  const report = jest.fn<(error: unknown) => void>();
  const preferences = new PromptEditorPreferences(comfy.app, report);
  preferences.setup();
  await settle();
  expect(preferences.current().allowed.emphasis).toBe(true);
  expect(report).toHaveBeenCalledWith(expect.any(TypeError));
});
