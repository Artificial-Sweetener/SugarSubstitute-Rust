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
/** Prove shared saving presentation never publishes drafts or stale reads as committed settings. */
import { jest } from '@jest/globals';
import {
  OutputSavingConfiguration,
  type OutputSavingCommands,
} from '../../../frontend/comfyui/ui/output_saving/OutputSavingConfiguration.js';
import { savingPreferences, savingSnapshot } from '../support/SavingSettingsFixture.js';

test('coalesces reads, publishes accepted saves and ignores an older GET response', async () => {
  let release: ((value: ReturnType<typeof savingSnapshot>) => void) | null = null;
  const load = jest.fn<OutputSavingCommands['load']>().mockImplementation(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  const saved = savingSnapshot({ ...savingPreferences, manage_workflow: true });
  const client: OutputSavingCommands = {
    load,
    save: async () => saved,
    importSubstitute: async () => saved,
    preview: async () => savingSnapshot(),
  };
  const configuration = new OutputSavingConfiguration(client);
  const changed = jest.fn();
  configuration.subscribe(changed);
  const first = configuration.load();
  expect(configuration.load()).toBe(first);
  await configuration.save(saved.preferences);
  if (!release) throw new Error('Missing pending settings request.');
  (release as (value: ReturnType<typeof savingSnapshot>) => void)(savingSnapshot());
  expect(await first).toBe(saved);
  expect(configuration.read()).toBe(saved);
  expect(load).toHaveBeenCalledTimes(1);
  expect(changed).toHaveBeenCalledTimes(1);
});

test('does not publish a proposed example or failed persistence as accepted settings', async () => {
  const snapshot = savingSnapshot();
  const configuration = new OutputSavingConfiguration({
    load: async () => snapshot,
    save: async () => {
      throw new Error('Disk unavailable');
    },
    importSubstitute: async () => snapshot,
    preview: async () => savingSnapshot({ ...savingPreferences, manage_workflow: true }),
  });
  await configuration.load();
  await configuration.preview(savingPreferences, new AbortController().signal);
  await expect(configuration.save(savingPreferences)).rejects.toThrow('Disk unavailable');
  expect(configuration.read()).toBe(snapshot);
});
