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
 * ComfyUI's own and other extensions' settings as workspace pages, so the SugarSubstitute frame
 * presents them as the same cards as its own. Pages, sections and order follow ComfyUI's dialog
 * (`useSettingUI.ts`, `SettingDialog.vue`): a page per first category level, a section per
 * second, sections by their highest sort order, rows by sort order with ties in reverse
 * registration order. Each row binds to ComfyUI's setting store; a custom-typed setting draws its
 * own control with its registered renderer.
 */
import { isRecord } from '../../types/common.js';
import type { ComfySettingDefinition, ComfySettingStore } from './ComfySettingStore.js';
import type {
  SettingsChoice,
  SettingsControl,
  SettingsIcon,
  SettingsPage,
  SettingsRow,
} from './SettingsModel.js';

/** Settings ComfyUI itself registers; any other category belongs to an extension. */
const COMFY_PREFIXES = ['Comfy.', 'LiteGraph.'];

/** A ComfyUI settings category presented as a page. */
export interface ComfySettingsPage extends SettingsPage {
  /** The category ComfyUI's dialog keys this page by, such as `root/Comfy`. */
  readonly navId: string;
  /** Whether ComfyUI registers the category's settings, rather than an extension. */
  readonly core: boolean;
}

/** Every setting category except `exclude`, as pages, in ComfyUI's tree order. */
export function comfySettingsPages(
  store: ComfySettingStore,
  exclude: readonly string[],
  icon: (navId: string) => SettingsIcon | undefined,
  title: (navId: string) => string | undefined = () => undefined,
): ComfySettingsPage[] {
  const pages = new Map<
    string,
    { core: boolean; sections: Map<string, ComfySettingDefinition[]> }
  >();
  for (const definition of store.definitions()) {
    if (definition.type === 'hidden' || definition.deprecated) continue;
    const path = definition.category?.length ? [...definition.category] : definition.id.split('.');
    const pageLabel = path.length > 1 ? (path[0] ?? 'Other') : 'Other';
    if (exclude.includes(pageLabel)) continue;
    const sectionLabel = path.length > 1 ? (path[1] ?? pageLabel) : pageLabel;
    let page = pages.get(pageLabel);
    if (!page) {
      page = { core: true, sections: new Map() };
      pages.set(pageLabel, page);
    }
    page.core &&= COMFY_PREFIXES.some((prefix) => definition.id.startsWith(prefix));
    const section = page.sections.get(sectionLabel) ?? [];
    section.push(definition);
    page.sections.set(sectionLabel, section);
  }
  return [...pages].map(([label, page]) => {
    const navId = `root/${label}`;
    const sections = [...page.sections]
      .map(([title, definitions]) => ({
        title,
        // Ties keep ComfyUI's order, which is the reverse of registration.
        definitions: [...definitions]
          .reverse()
          .sort((a, b) => (b.sortOrder ?? 0) - (a.sortOrder ?? 0)),
      }))
      .sort(
        (a, b) =>
          Math.max(...b.definitions.map((d) => d.sortOrder ?? 0)) -
            Math.max(...a.definitions.map((d) => d.sortOrder ?? 0)) ||
          a.title.localeCompare(b.title),
      )
      .map((section) => ({
        id: `${navId}/${section.title}`,
        title: spaced(section.title),
        rows: section.definitions.map((definition) => comfySettingRow(store, definition)),
      }));
    const pageIcon = icon(navId);
    return {
      id: navId,
      navId,
      core: page.core,
      title: title(navId) ?? spaced(label),
      subtitle: '',
      icon: pageIcon ?? 'settings',
      sections,
    };
  });
}

/**
 * A category key as ComfyUI's English labels read it: words joined in camel case are spaced
 * (`DevMode` is "Dev Mode"); acronyms and numbers stay as they are.
 */
export function spaced(key: string): string {
  return key.replace(/([a-z])([A-Z])/g, '$1 $2');
}

/** One ComfyUI setting as a workspace row bound to the store. */
export function comfySettingRow(
  store: ComfySettingStore,
  definition: ComfySettingDefinition,
): SettingsRow {
  const control = comfySettingControl(store, definition);
  return {
    id: definition.id,
    title: definition.name ?? definition.id,
    ...(definition.tooltip ? { description: definition.tooltip } : {}),
    keywords: [definition.id],
    ...(control ? { controls: [control] } : {}),
    ...(definition.type === 'combo' || definition.type === 'radio' ? { wrap: 640 } : {}),
  };
}

function attr(definition: ComfySettingDefinition, name: string): number | undefined {
  const value = definition.attrs?.[name];
  return typeof value === 'number' ? value : undefined;
}

/** A combo's choices, keeping each option's own value type for writing back. */
function choices(
  definition: ComfySettingDefinition,
  current: unknown,
): { choice: SettingsChoice; value: unknown }[] {
  const options: unknown =
    typeof definition.options === 'function'
      ? (definition.options as (value: unknown) => unknown)(current)
      : definition.options;
  if (!Array.isArray(options)) return [];
  return options.map((option: unknown) => {
    if (isRecord(option) && 'value' in option)
      return {
        choice: { value: String(option.value), label: String(option.text ?? option.value) },
        value: option.value,
      };
    return { choice: { value: String(option), label: String(option) }, value: option };
  });
}

function comfySettingControl(
  store: ComfySettingStore,
  definition: ComfySettingDefinition,
): SettingsControl | null {
  const { id, type } = definition;
  const subscribe = (listener: () => void): (() => void) => store.subscribe(id, listener);
  if (typeof type === 'function') {
    const render = type as (
      name: string,
      setter: (value: unknown) => void,
      value: unknown,
      attrs?: unknown,
    ) => unknown;
    return {
      kind: 'element',
      create: (document) => {
        const drawn = render(
          definition.name ?? id,
          (value) => void store.set(id, value),
          store.get(id),
          definition.attrs,
        );
        return drawn instanceof HTMLElement ? drawn : document.createElement('span');
      },
    };
  }
  switch (type) {
    case 'boolean':
      return {
        kind: 'switch',
        value: {
          read: () => store.get(id) === true,
          write: (value) => store.set(id, value),
          subscribe,
        },
      };
    case 'combo':
    case 'radio':
      return {
        kind: 'choice',
        value: {
          read: () => String(store.get(id)),
          write: (value) => {
            const match = choices(definition, store.get(id)).find(
              (option) => option.choice.value === value,
            );
            return store.set(id, match ? match.value : value);
          },
          subscribe,
        },
        choices: {
          read: () => choices(definition, store.get(id)).map((option) => option.choice),
          subscribe,
        },
      };
    case 'number':
    case 'knob':
    case 'slider': {
      const min = attr(definition, 'min');
      const max = attr(definition, 'max');
      const step = attr(definition, 'step');
      const value = {
        read: () => {
          const stored = store.get(id);
          return typeof stored === 'number' ? stored : Number(stored ?? 0);
        },
        write: (next: number) => store.set(id, next),
        subscribe,
      };
      return type === 'slider'
        ? { kind: 'slider', value, min: min ?? 0, max: max ?? 100, step: step ?? 1 }
        : {
            kind: 'number',
            value,
            ...(min !== undefined ? { min } : {}),
            ...(max !== undefined ? { max } : {}),
            ...(step !== undefined ? { step } : {}),
          };
    }
    case 'color':
      return {
        kind: 'element',
        create: (document) => colorControl(document, store, id),
      };
    default:
      return {
        kind: 'text',
        value: {
          read: () => {
            const stored = store.get(id);
            return stored === undefined || stored === null ? '' : String(stored);
          },
          write: (value) => store.set(id, value),
          subscribe,
        },
      };
  }
}

/** ComfyUI's colour setting: a hex value stored without `#`. */
function colorControl(document: Document, store: ComfySettingStore, id: string): HTMLElement {
  const input = document.createElement('input');
  input.type = 'color';
  input.className = 'sc-sw-color';
  const read = (): string => {
    const value = store.get(id);
    return typeof value === 'string' && /^[0-9a-f]{6}$/i.test(value) ? `#${value}` : '#000000';
  };
  input.value = read();
  input.addEventListener('change', () => void store.set(id, input.value.slice(1)));
  return input;
}
