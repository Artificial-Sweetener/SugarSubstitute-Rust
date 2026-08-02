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
/** Provide a deterministic Comfy Settings select boundary for DOM tests. */

import type {
  ComfySettingsAutocompleteProps,
  ComfySettingsSelectMount,
  ComfySettingsSelectRenderer,
  ComfySettingsSingleSelectProps,
} from '../../../frontend/comfyui/ui/controls/ComfySettingsSelect.js';

/** Render semantic test triggers while retaining each native component contract. */
export class TestComfySettingsSelectRenderer implements ComfySettingsSelectRenderer {
  readonly #autocomplete = new Map<string, ComfySettingsAutocompleteProps>();
  readonly #single = new Map<string, ComfySettingsSingleSelectProps>();

  /** Mount a test representation of Comfy's SingleSelect trigger. */
  mountSingle(
    target: HTMLElement,
    props: ComfySettingsSingleSelectProps,
  ): ComfySettingsSelectMount<ComfySettingsSingleSelectProps> {
    const button = target.ownerDocument.createElement('button');
    button.type = 'button';
    button.setAttribute('role', 'combobox');
    const update = (nextProps: ComfySettingsSingleSelectProps): void => {
      this.#single.set(nextProps.ariaLabel, nextProps);
      button.setAttribute('aria-label', nextProps.ariaLabel);
      button.disabled = nextProps.disabled ?? false;
      button.textContent =
        nextProps.options.find((option) => option.value === nextProps.value)?.label ??
        nextProps.ariaLabel;
    };
    update(props);
    target.replaceChildren(button);
    return {
      update,
      unmount: () => {
        if (target.contains(button)) target.replaceChildren();
      },
    };
  }

  /** Mount a test representation of Comfy's editable AutoComplete. */
  mountAutocomplete(
    target: HTMLElement,
    props: ComfySettingsAutocompleteProps,
  ): ComfySettingsSelectMount<ComfySettingsAutocompleteProps> {
    const input = target.ownerDocument.createElement('input');
    input.setAttribute('role', 'combobox');
    const update = (nextProps: ComfySettingsAutocompleteProps): void => {
      this.#autocomplete.set(nextProps.ariaLabel, nextProps);
      input.setAttribute('aria-label', nextProps.ariaLabel);
      input.disabled = nextProps.disabled ?? false;
      input.placeholder = nextProps.placeholder ?? '';
      input.value = nextProps.values.join(', ');
    };
    update(props);
    target.replaceChildren(input);
    return {
      update,
      unmount: () => {
        if (target.contains(input)) target.replaceChildren();
      },
    };
  }

  /** Emit one native SingleSelect value update. */
  selectSingle(label: string, value: string): void {
    const props = this.#single.get(label);
    if (!props) throw new Error(`Missing Settings SingleSelect: ${label}`);
    props.onChange(value);
  }

  /** Emit one native AutoComplete value update, including arbitrary entries. */
  enterAutocomplete(label: string, values: readonly string[]): void {
    const props = this.#autocomplete.get(label);
    if (!props) throw new Error(`Missing Settings AutoComplete: ${label}`);
    props.onChange(values);
  }

  /** Ask the AutoComplete to refresh suggestions for a query. */
  completeAutocomplete(label: string, query: string): void {
    const props = this.#autocomplete.get(label);
    if (!props) throw new Error(`Missing Settings AutoComplete: ${label}`);
    props.onComplete(query);
  }

  /** Read the most recent SingleSelect contract. */
  single(label: string): ComfySettingsSingleSelectProps {
    const props = this.#single.get(label);
    if (!props) throw new Error(`Missing Settings SingleSelect: ${label}`);
    return props;
  }

  /** Read the most recent AutoComplete contract. */
  autocomplete(label: string): ComfySettingsAutocompleteProps {
    const props = this.#autocomplete.get(label);
    if (!props) throw new Error(`Missing Settings AutoComplete: ${label}`);
    return props;
  }
}
