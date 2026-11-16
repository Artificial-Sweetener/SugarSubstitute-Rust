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
/** Offer the wildcard folder's file actions in SugarCubes' settings. */
import { sugarCubesSettingsSection } from '../../settings/SugarCubesSettingsSections.js';
import {
  applySettingsControlsStyle,
  applySettingsRowShellStyle,
  applySettingsSummaryStyle,
} from '../../settings/SettingsRowPresentation.js';
import { isRecord } from '../../types/common.js';
import type { WildcardFile } from '../domain/WildcardSource.js';

/** The setting row holding the wildcard folder's actions. */
export const WILDCARD_SETTING = 'SugarCubes.Prompts.Wildcards';

/** Ask the person for one line of text; null when they cancel. */
export type WildcardNamePrompt = (options: {
  title: string;
  label: string;
  helperText: string;
  confirmLabel: string;
  initialValue?: string;
  validate(value: string): string | null;
}) => Promise<string | null>;

export interface WildcardSettingsOptions {
  readonly app: unknown;
  readonly document: Document;
  /** The folder as last listed. */
  files(): readonly WildcardFile[];
  refresh(): Promise<void>;
  /** Admit a path inside the folder as a wildcard file, or null. */
  admit(path: string): WildcardFile | null;
  create(path: string, text: string): Promise<'created' | 'exists'>;
  rename(from: string, to: string): Promise<'renamed' | 'exists' | 'missing'>;
  openFolder(): Promise<void>;
  prompt: WildcardNamePrompt;
  report(error: unknown): void;
}

const SUMMARY =
  'Wildcards are .txt and .csv files in the sugar/wildcards folder of your ComfyUI user folder. ' +
  'A prompt names one as {colors}, {people/hair}, {csv:people:hair} or {colors|tag}, and each ' +
  'run picks from it with the run’s seed.';

/**
 * Register the Wildcards row: open the folder, create a wildcard, rename a wildcard.
 *
 * A name is written as a prompt writes it, so `people/hair` creates `people/hair.txt`; ending the
 * name in `.csv` creates a CSV wildcard. Names that are taken or are not wildcard files are
 * refused in the dialog, before anything is written.
 */
export function registerWildcardSettings(options: WildcardSettingsOptions): void {
  const host = isRecord(options.app) ? options.app : {};
  const ui = isRecord(host.ui) ? host.ui : {};
  const settings = isRecord(ui.settings) ? ui.settings : null;
  if (!settings || typeof settings.addSetting !== 'function') {
    options.report(new Error('Comfy settings registration is unavailable for wildcards.'));
    return;
  }
  let row: HTMLElement | null = null;
  (settings.addSetting as (setting: unknown) => unknown).call(settings, {
    id: WILDCARD_SETTING,
    name: 'Wildcards',
    ...sugarCubesSettingsSection('prompts', 'Wildcards'),
    defaultValue: '',
    tooltip: 'Open the wildcard folder, or create and rename wildcard files.',
    type: () => (row ??= wildcardSettingsRow(options)),
  });
}

/** Build the row once; its actions read the folder as it is when they run. */
function wildcardSettingsRow(options: WildcardSettingsOptions): HTMLElement {
  const { document } = options;
  const root = document.createElement('div');
  root.className = 'sugarcubes-settings-row sugarcubes-settings-row--wildcards';
  applySettingsRowShellStyle(root);
  const controls = document.createElement('div');
  applySettingsControlsStyle(controls);
  const action = (label: string, run: () => Promise<void>): void => {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'p-button p-component p-button-sm p-button-text p-button-secondary';
    const text = document.createElement('span');
    text.className = 'p-button-label';
    text.textContent = label;
    button.append(text);
    button.addEventListener('click', () => {
      button.disabled = true;
      void run()
        .catch(options.report)
        .finally(() => {
          button.disabled = false;
        });
    });
    controls.append(button);
  };
  action('Open wildcards folder', () => options.openFolder());
  action('New wildcard…', () => createWildcard(options));
  action('Rename wildcard…', () => renameWildcard(options));
  const summary = document.createElement('div');
  applySettingsSummaryStyle(summary);
  summary.textContent = SUMMARY;
  root.append(controls, summary);
  return root;
}

/** Turn a typed name into a file path: a TXT wildcard unless the name ends in `.csv` or `.txt`. */
function pathOf(name: string, extension = '.txt'): string {
  const trimmed = name.trim().replaceAll('\\', '/');
  return /\.(?:txt|csv)$/iu.test(trimmed) ? trimmed : trimmed + extension;
}

/** Find the file a typed name refers to: its path, or its name as a prompt writes it. */
function named(files: readonly WildcardFile[], name: string): WildcardFile | null {
  const typed = name.trim().replaceAll('\\', '/').toLowerCase();
  return (
    files.find((file) => file.path.toLowerCase() === typed) ??
    files.find((file) => file.identifier === typed) ??
    null
  );
}

/** Why a new file cannot take this name, or null when it can. */
function refusal(options: WildcardSettingsOptions, path: string): string | null {
  const file = options.admit(path);
  if (!file) return 'Use letters, numbers and folders, such as colors or people/hair.';
  return options
    .files()
    .some((existing) => existing.kind === file.kind && existing.identifier === file.identifier)
    ? 'A wildcard with this name already exists.'
    : null;
}

/** Ask for a name and create an empty wildcard file with it. */
async function createWildcard(options: WildcardSettingsOptions): Promise<void> {
  await options.refresh();
  const name = await options.prompt({
    title: 'New wildcard',
    label: 'Name',
    helperText:
      'As a prompt will write it: colors, or people/hair. End it in .csv for a CSV wildcard.',
    confirmLabel: 'Create',
    validate: (value) => refusal(options, pathOf(value)),
  });
  if (name === null) return;
  if ((await options.create(pathOf(name), '')) === 'exists')
    throw new Error('A wildcard with this name already exists.');
  await options.refresh();
}

/** Ask which wildcard to rename and what to call it, then rename its file. */
async function renameWildcard(options: WildcardSettingsOptions): Promise<void> {
  await options.refresh();
  const current = await options.prompt({
    title: 'Rename wildcard',
    label: 'Wildcard to rename',
    helperText: 'The name a prompt uses for it now, such as colors or people/hair.',
    confirmLabel: 'Next',
    validate: (value) =>
      named(options.files(), value) ? null : 'There is no wildcard with this name.',
  });
  const file = current === null ? null : named(options.files(), current);
  if (!file) return;
  const extension = file.path.slice(file.path.lastIndexOf('.'));
  const name = await options.prompt({
    title: 'Rename wildcard',
    label: `New name for ${file.identifier}`,
    helperText: 'Prompts that use the old name must be changed to the new one.',
    confirmLabel: 'Rename',
    initialValue: file.path.slice(0, file.path.length - extension.length),
    validate: (value) => refusal(options, pathOf(value, extension)),
  });
  if (name === null) return;
  const outcome = await options.rename(file.path, pathOf(name, extension));
  if (outcome === 'exists') throw new Error('A wildcard with this name already exists.');
  if (outcome === 'missing') throw new Error('That wildcard is no longer in the folder.');
  await options.refresh();
}
