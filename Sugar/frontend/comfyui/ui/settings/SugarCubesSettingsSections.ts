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
 * Organize SugarCubes and SugarSubstitute settings as the Python SugarSubstitute did: pages in its
 * navigation order, each holding titled sections. Every setting is registered with ComfyUI under
 * its section, so ComfyUI keeps owning values, validation and search; the settings navigation
 * (`SettingsPageNavigation`) presents the sections as pages.
 */
import type { FluentIconName } from '../core/FluentIcons.js';

/** One settings page and its sections, in the Python app's order. */
export interface SettingsPage {
  readonly id: string;
  readonly title: string;
  readonly subtitle: string;
  readonly icon: FluentIconName;
  readonly sections: readonly string[];
}

/** The pages, in the Python app's navigation order; pages with no settings today are left out. */
export const SETTINGS_PAGES: readonly SettingsPage[] = [
  {
    id: 'about',
    title: 'About',
    subtitle: 'Version, project, updates and acknowledgements.',
    icon: 'app-generic',
    sections: ['About', 'Updates'],
  },
  {
    id: 'generation',
    title: 'Generation',
    subtitle: 'Generation behavior and generated files.',
    icon: 'play',
    sections: ['Output canvas', 'Output files', 'Model updates', 'Memory'],
  },
  {
    id: 'prompt_editing',
    title: 'Prompt Editing',
    subtitle: 'Prompt editor behavior and authoring support.',
    icon: 'edit',
    sections: ['Wildcards'],
  },
  {
    id: 'controls',
    title: 'Controls',
    subtitle: 'Keyboard and pointer controls.',
    icon: 'settings',
    sections: ['Workflow connections'],
  },
  {
    id: 'model_sources',
    title: 'Model Sources',
    subtitle: 'External providers, metadata, previews and caches.',
    icon: 'brain-circuit',
    sections: ['Model library', 'Model metadata', 'Model previews'],
  },
  {
    id: 'library',
    title: 'Library',
    subtitle: 'Cube Packs, their updates and authoring.',
    icon: 'folder',
    sections: ['Cube library', 'Authoring'],
  },
  {
    id: 'appearance',
    title: 'Appearance',
    subtitle: 'Visual customization.',
    icon: 'color',
    sections: ['Style', 'Cube stack'],
  },
];

/** Where each SugarCubes setting family belongs: its section label. */
export const sugarCubesSettingsSections = {
  about: 'About',
  updates: 'Updates',
  canvas: 'Output canvas',
  output: 'Output files',
  modelUpdates: 'Model updates',
  memory: 'Memory',
  prompts: 'Wildcards',
  connections: 'Workflow connections',
  models: 'Model library',
  metadata: 'Model metadata',
  previews: 'Model previews',
  packs: 'Cube library',
  authoring: 'Authoring',
  appearance: 'Style',
  stack: 'Cube stack',
} as const;

/** The section order across all pages, first to last. */
const SECTION_ORDER: readonly string[] = SETTINGS_PAGES.flatMap((page) => page.sections);

/**
 * Place one setting: under the SugarCubes category, in its section. ComfyUI shows higher
 * `sortOrder` first, so the Python order counts down.
 */
export function sugarCubesSettingsSection(
  section: keyof typeof sugarCubesSettingsSections,
  key: string,
): { category: string[]; sortOrder: number } {
  const label = sugarCubesSettingsSections[section];
  return {
    category: ['SugarCubes', label, key],
    sortOrder: 10_000 - SECTION_ORDER.indexOf(label) * 10,
  };
}

/** The page a section belongs to. */
export function settingsPageOfSection(label: string): SettingsPage | null {
  return SETTINGS_PAGES.find((page) => page.sections.includes(label)) ?? null;
}
