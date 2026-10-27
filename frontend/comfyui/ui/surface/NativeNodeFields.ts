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
import { NativeFieldActionMount } from './NativeFieldActionMount.js';
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
/** Describe a numeric control rendered and styled by the existing node-field pipeline. */
export interface NativeNodeNumberMount {
  update(value: number, disabled?: boolean, visible?: boolean): void;
  dispose(): void;
}
/** Retain a read-only native value and its trailing action as one ordinary field. */
export interface NativeNodeActionMount {
  update(value: string, disabled?: boolean, visible?: boolean, title?: string): void;
  dispose(): void;
}
interface Field {
  readonly widget: ComfyWidget;
  readonly callbacks: Set<(value: unknown) => void>;
  signature: string;
  release(): void;
}
/** Keep display-only widgets out of serialization while the native node owns their rendering and theme. */
export class NativeNodeFields {
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
    const owner = this.#field(label, key, 'combo');
    const release = this.#retain(owner, (value) => {
      if (typeof value === 'string') change(value);
    });
    const name = owner.widget.name;
    let constraints: NativeChoiceRestrictions | null = null,
      preparation: NativeChoicePreparation | null = null,
      anchor: HTMLElement | null = null,
      disposed = false;
    return {
      update: (value, choices, disabled = false, visible = true) => {
        if (disposed) return;
        const options = owner.widget.options;
        if (!options) throw new Error('Comfy native field options are unavailable.');
        this.#update(owner, value, [choices, disabled, visible], {
          hidden: !visible,
          disabled,
          values: choices.map((choice) => choice.value),
          allowedValues: choices
            .filter((choice) => !choice.blockers.length)
            .map((choice) => choice.value),
          getOptionLabel: (value: string) =>
            choices.find((choice) => choice.value === value)?.label ?? value,
        });
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
        release();
      },
    };
  }

  /** Mount native numeric fields with optional shared spinner-slider presentation. */
  mountNumber(
    label: string,
    change: (value: number) => void,
    range: {
      readonly min: number;
      readonly max: number;
      readonly step: number;
      readonly precision: number;
      readonly presentation?: 'spinner-slider';
    },
    key = label,
  ): NativeNodeNumberMount {
    const owner = this.#field(label, key, 'number');
    const release = this.#retain(owner, (value) => {
      if (typeof value === 'number' && Number.isFinite(value)) change(value);
    });
    let disposed = false;
    return {
      update: (value, disabled = false, visible = true) => {
        if (!disposed)
          this.#update(owner, value, [range, disabled, visible], {
            ...range,
            // LiteGraph's legacy step is ten times the actual increment; Nodes 2 reads step2.
            step: range.step * 10,
            step2: range.step,
            hidden: !visible,
            disabled,
          });
      },
      dispose: () => {
        if (disposed) return;
        disposed = true;
        release();
      },
    };
  }

  /** Present a read-only text value and shared command inside Comfy's own field row. */
  mountActionField(
    root: HTMLElement,
    label: string,
    actionLabel: string,
    action: () => void,
    key = label,
  ): NativeNodeActionMount {
    const owner = this.#field(label, key, 'string');
    const release = this.#retain(owner, () => {});
    const command = new NativeFieldActionMount(root, actionLabel, action);
    let disposed = false;
    return {
      update: (value, disabled = false, visible = true, title = value) => {
        if (disposed) return;
        this.#update(owner, value, [disabled, visible, title], {
          read_only: true,
          title,
          hidden: !visible,
          disabled,
        });
        command.update(owner.widget.name, disabled || !visible);
      },
      dispose: () => {
        if (disposed) return;
        disposed = true;
        command.dispose();
        release();
      },
    };
  }

  /** Construct each native field once, preserving the live node's graph-widget array. */
  #field(label: string, key: string, type: 'combo' | 'number' | 'string'): Field {
    const name = '__sugarcubes_presentation:' + this.namespace + ':' + key;
    let field = this.#fields.get(name);
    if (!field) {
      const callbacks = new Set<(value: unknown) => void>();
      const add = this.node.addCustomWidget;
      if (typeof add !== 'function') throw new Error('Comfy native node fields are unavailable.');
      const original = this.node.widgets,
        items = [...(original ?? [])];
      let widget: unknown;
      try {
        widget = add.call(this.node, {
          name,
          label,
          type,
          value: type === 'number' ? 0 : '',
          serialize: false,
          options: { serialize: false, ...(type === 'combo' ? { values: [] } : {}) },
          callback: (value: unknown) => {
            callbacks.values().next().value?.(value);
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
    if (field.widget.type !== type) throw new Error('The native presentation field type changed.');
    return field;
  }

  /** Share one active commit owner and one native presentation lease across simultaneous views. */
  #retain(owner: Field, change: (value: unknown) => void): () => void {
    const field = owner;
    if (!field.callbacks.size)
      field.release = registerNativePresentationWidget(this.node, field.widget, this.beforeField);
    field.callbacks.add(change);
    return () => {
      owner.callbacks.delete(change);
      if (owner.callbacks.size) return;
      owner.release();
      owner.signature = '';
      // Comfy retains registered widget state until the graph closes; reuse its exact options identity.
      const options = owner.widget.options;
      if (options && owner.widget.type === 'combo') {
        options.values = [];
        options.allowedValues = [];
        delete options.getOptionLabel;
      }
      owner.widget.value = '';
      owner.widget.hidden = true;
      owner.widget.disabled = true;
      this.#refresh();
    };
  }

  /** Synchronize accepted values without replacing options retained by native widget stores. */
  #update(
    owner: Field,
    value: string | number,
    details: readonly unknown[],
    options: Record<string, unknown>,
  ): void {
    if (owner.widget.value !== value) owner.widget.value = value;
    const signature = JSON.stringify([value, details]);
    if (owner.signature === signature) return;
    owner.signature = signature;
    if (!owner.widget.options) throw new Error('Comfy native field options are unavailable.');
    Object.assign(owner.widget.options, { serialize: false, ...options });
    owner.widget.hidden = options.hidden;
    owner.widget.disabled = options.disabled;
    this.#refresh();
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
