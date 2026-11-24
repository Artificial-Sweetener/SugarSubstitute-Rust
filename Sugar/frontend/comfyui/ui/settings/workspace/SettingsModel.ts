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
 * The settings workspace's model: pages of sections of rows, as the Python SugarSubstitute's
 * Settings presents them. A row's text and place come from the layout; its controls come from
 * the feature that owns the value, bound to wherever that value lives.
 */
import type { FluentIconName } from '../../core/FluentIcons.js';

/** A live value the workspace reads, writes and follows. */
export interface SettingsValue<T> {
  read(): T;
  write(value: T): void | Promise<void>;
  subscribe(listener: () => void): () => void;
}

/** A live fact the workspace reads and follows, such as a row's enabled state or a status. */
export interface SettingsFact<T> {
  read(): T;
  subscribe(listener: () => void): () => void;
}

/** An icon: a vendored Fluent icon by name, or an element the feature draws. */
export type SettingsIcon = FluentIconName | ((document: Document) => HTMLElement);

/** One choice of a choice control. */
export interface SettingsChoice {
  readonly value: string;
  readonly label: string;
}

/** A button in a row. */
export interface SettingsButton {
  readonly label: string;
  run(): void | Promise<void>;
  readonly enabled?: SettingsFact<boolean>;
  readonly primary?: boolean;
}

/** A row's trailing control; several make a control group. */
export type SettingsControl =
  | { readonly kind: 'switch'; readonly value: SettingsValue<boolean> }
  | {
      readonly kind: 'choice';
      readonly value: SettingsValue<string>;
      readonly choices: SettingsFact<readonly SettingsChoice[]>;
      readonly width?: number;
    }
  | {
      readonly kind: 'number';
      readonly value: SettingsValue<number>;
      readonly min?: number;
      readonly max?: number;
      readonly step?: number;
    }
  | {
      readonly kind: 'slider';
      readonly value: SettingsValue<number>;
      readonly min: number;
      readonly max: number;
      readonly step: number;
    }
  | {
      readonly kind: 'text';
      readonly value: SettingsValue<string>;
      readonly placeholder?: string;
      readonly width?: number;
      readonly readOnly?: boolean;
    }
  | { readonly kind: 'button'; readonly button: SettingsButton }
  | { readonly kind: 'status'; readonly text: SettingsFact<string> }
  | { readonly kind: 'element'; create(document: Document): HTMLElement; dispose?(): void };

/** The controls and behaviour a feature gives one row of the layout. */
export interface SettingsRowContent {
  readonly controls?: readonly SettingsControl[];
  /** A switch expander: the header switch, and the rows shown while it is on. */
  readonly expander?: {
    readonly switch?: SettingsValue<boolean>;
    readonly rows: readonly SettingsRow[];
  };
  /** Hidden rows are removed; a section with no visible rows disappears with its title. */
  readonly visible?: SettingsFact<boolean>;
  /** Only the controls are disabled; the card's text keeps its colours. */
  readonly enabled?: SettingsFact<boolean>;
  /** A row drawn entirely by its feature (the About and Library page bodies). */
  readonly body?: (document: Document) => HTMLElement;
}

/** One row as the workspace draws it. */
export interface SettingsRow extends SettingsRowContent {
  readonly id: string;
  readonly title: string;
  readonly description?: string;
  readonly icon?: SettingsIcon;
  readonly keywords?: readonly string[];
  /** Below this card width the controls move under the text. */
  readonly wrap?: number;
}

/** A titled group of rows. */
export interface SettingsSection {
  readonly id: string;
  readonly title: string;
  readonly subtitle?: string;
  readonly rows: readonly SettingsRow[];
}

/** One page of the workspace. */
export interface SettingsPage {
  readonly id: string;
  readonly title: string;
  readonly subtitle: string;
  readonly icon: SettingsIcon;
  readonly sections: readonly SettingsSection[];
  /** Pages whose rows Python's search does not index (About, Library). */
  readonly searchable?: boolean;
}

/** A fact that never changes. */
export function fixed<T>(value: T): SettingsFact<T> {
  return { read: () => value, subscribe: () => () => undefined };
}

/** A value and its listeners held in memory, for state a feature computes. */
export class SettingsState<T> implements SettingsValue<T> {
  readonly #listeners = new Set<() => void>();
  #current: T;

  constructor(
    initial: T,
    private readonly commit: (value: T) => void | Promise<void> = () => undefined,
  ) {
    this.#current = initial;
  }

  read(): T {
    return this.#current;
  }

  write(value: T): void | Promise<void> {
    this.set(value);
    return this.commit(value);
  }

  /** Change the value without committing it, as when its owner reports a new value. */
  set(value: T): void {
    if (Object.is(value, this.#current)) return;
    this.#current = value;
    for (const listener of [...this.#listeners]) listener();
  }

  subscribe(listener: () => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }
}
