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
/** Lease presentation-only fields through Comfy's actual node widget lifecycle. */
import type { ComfyNode, ComfyWidget } from '../types/graph.js';
import { isRecord } from '../types/common.js';
import { nativeNodeFieldRows } from './NativeNodeFieldRows.js';
import {
  NativeChoiceRestrictions,
  type NativeChoiceRestriction,
} from './NativeChoiceRestrictions.js';
import { registerNativePresentationWidget } from './NativePresentationWidgets.js';
import { refreshNativeNodeCards } from './NativeNodeCardRefresh.js';
import { NativeChoicePreparation } from './NativeChoicePreparation.js';
export interface NativeNodeChoiceOption extends NativeChoiceRestriction {
  label: string;
}

export interface NativeNodeChoiceMount {
  update(
    value: string,
    choices: readonly NativeNodeChoiceOption[],
    disabled?: boolean,
    visible?: boolean,
  ): void;
  dispose(): void;
}
interface Field {
  readonly widget: ComfyWidget;
  readonly callbacks: Set<(value: string) => void>;
  signature: string;
  release(): void;
}
/** Keep display-only widgets out of serialization while the native node owns their rendering and theme. */
export class NativeNodeChoiceFields {
  readonly #fields = new Map<string, Field>();
  #pending = false;
  /** Scope widget identities and optional placement before an existing native field. */
  constructor(
    private readonly node: ComfyNode,
    private readonly namespace: string,
    private readonly document: Document,
    private readonly beforeField: string | null = null,
  ) {}

  /** Share a native field across simultaneous projections without multiplying value commits. */
  mountChoice(
    root: HTMLElement,
    label: string,
    change: (value: string) => void,
    key = label,
    prepare?: () => Promise<boolean>,
  ): NativeNodeChoiceMount {
    const name = '__sugarcubes_presentation:' + this.namespace + ':' + key;
    let field = this.#fields.get(name);
    if (!field) {
      const callbacks = new Set<(value: string) => void>();
      const add = this.node.addCustomWidget;
      if (typeof add !== 'function') throw new Error('Comfy native node fields are unavailable.');
      const original = this.node.widgets,
        items = [...(original ?? [])];
      let widget: unknown;
      try {
        widget = add.call(this.node, {
          name,
          label,
          type: 'combo',
          value: '',
          serialize: false,
          options: { serialize: false, values: [] },
          callback: (value: unknown) => {
            if (typeof value === 'string') callbacks.values().next().value?.(value);
          },
        });
      } finally {
        if (original) original.splice(0, original.length, ...items);
        if (original) this.node.widgets = original;
        else delete this.node.widgets;
      }
      if (!isNativeWidget(widget)) throw new Error('Comfy returned an invalid native field.');
      widget.serialize = false;
      widget.label = label;
      field = {
        widget,
        callbacks,
        signature: '',
        release: () => {},
      };
      this.#fields.set(name, field);
    }
    if (!field.callbacks.size)
      field.release = registerNativePresentationWidget(this.node, field.widget, this.beforeField);
    field.callbacks.add(change);
    const owner = field;
    let constraints: NativeChoiceRestrictions | null = null,
      preparation: NativeChoicePreparation | null = null,
      anchor: HTMLElement | null = null,
      disposed = false;
    return {
      update: (value, choices, disabled = false, visible = true) => {
        if (disposed) return;
        // Native controls update optimistically; rejected intents must restore the binding's accepted value.
        if (owner.widget.value !== value) owner.widget.value = value;
        const signature = JSON.stringify([value, choices, disabled, visible]);
        if (owner.signature !== signature) {
          owner.signature = signature;
          const options = owner.widget.options;
          if (!options) throw new Error('Comfy native field options are unavailable.');
          // Native WidgetValueStore retains this options object when the widget registers.
          Object.assign(options, {
            serialize: false,
            hidden: !visible,
            disabled,
            values: choices.map((choice) => choice.value),
            allowedValues: choices
              .filter((choice) => !choice.blockers.length)
              .map((choice) => choice.value),
            getOptionLabel: (value: string) =>
              choices.find((choice) => choice.value === value)?.label ?? value,
          });
          owner.widget.hidden = !visible;
          owner.widget.disabled = disabled;
          this.#refresh();
        }
        const row = nativeNodeFieldRows(root).get(name) ?? null;
        if (anchor !== row) {
          constraints?.dispose();
          preparation?.dispose();
          anchor = row;
          constraints = row ? new NativeChoiceRestrictions(row) : null;
          preparation = row && prepare ? new NativeChoicePreparation(row, prepare) : null;
        }
        constraints?.update(
          choices.map((choice) => ({ value: choice.label, blockers: choice.blockers })),
        );
      },
      dispose: () => {
        if (disposed) return;
        disposed = true;
        constraints?.dispose();
        preparation?.dispose();
        owner.callbacks.delete(change);
        if (owner.callbacks.size) return;
        owner.release();
        owner.signature = '';
        // Comfy retains registered widget state until the graph closes; reuse its exact options identity.
        const options = owner.widget.options;
        if (options) {
          options.values = [];
          options.allowedValues = [];
          delete options.getOptionLabel;
        }
        owner.widget.value = '';
        owner.widget.hidden = true;
        owner.widget.disabled = true;
        this.#refresh();
      },
    };
  }

  /** Coalesce paired source/layer changes into one scoped native card refresh. */
  #refresh(): void {
    if (this.#pending) return;
    this.#pending = true;
    queueMicrotask(() => {
      this.#pending = false;
      refreshNativeNodeCards(this.document, { kind: 'nodes', nodes: [this.node] });
    });
  }
}
/** Admit the native widget without replacing its reactive identity or property descriptors. */
function isNativeWidget(value: unknown): value is ComfyWidget {
  return isRecord(value) && typeof value.name === 'string';
}
