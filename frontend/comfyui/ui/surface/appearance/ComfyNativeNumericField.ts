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
/** Adapt recognized Comfy numeric components to their native reactive value owner. */
import { isRecord } from '../../types/common.js';
import type { UnknownRecord } from '../../types/common.js';
import { findVueComponents } from '../ComfyVueTree.js';
import type { NumericSliderSpec } from './NumericSliderPresentation.js';

export interface NativeNumericField extends NumericSliderSpec {
  row: HTMLElement;
  container: HTMLElement;
  spinner: HTMLElement;
  input: HTMLInputElement;
  increment: HTMLButtonElement | null;
  decrement: HTMLButtonElement | null;
  rangeFill: HTMLElement | null;
  hasNativeSlider: boolean;
  readValue(): number | null;
  disabled(): boolean;
  writeValue(value: number): void;
  subscribe(listener: () => void): () => void;
}

/** Accept only known native numeric widgets and their own input component within this row. */
export function readNativeNumericField(
  row: HTMLElement,
  widgetVNode: UnknownRecord,
): NativeNumericField | null {
  const component = isRecord(widgetVNode.component) ? widgetVNode.component : {};
  const props = isRecord(component.props) ? component.props : {};
  const widget = isRecord(props.widget) ? props.widget : {};
  if (typeof widget.name !== 'string') return null;
  const numericVNode = [
    ...findVueComponents(widgetVNode, 'ScrubableNumberInput'),
    ...findVueComponents(widgetVNode, 'InputNumber'),
  ].find((vnode) => {
    const instance = isRecord(vnode.component) ? vnode.component : {};
    const tree = isRecord(instance.subTree) ? instance.subTree : {};
    return tree.el instanceof HTMLElement && row.contains(tree.el);
  });
  const numeric = isRecord(numericVNode?.component) ? numericVNode.component : {};
  const numericProps = isRecord(numeric.props) ? numeric.props : {};
  const tree = isRecord(numeric.subTree) ? numeric.subTree : {};
  const proxy = isRecord(numeric.proxy) ? numeric.proxy : {};
  const emit = numeric.emit;
  const watch = proxy.$watch;
  const spinner = tree.el;
  if (
    !(spinner instanceof HTMLElement) ||
    typeof emit !== 'function' ||
    typeof watch !== 'function'
  )
    return null;
  const input = spinner.querySelector<HTMLInputElement>('input[role="spinbutton"]');
  const layout = row.querySelector<HTMLElement>('[data-sc-style-layout]');
  const container = layout?.lastElementChild;
  if (!input || !(container instanceof HTMLElement) || !container.contains(spinner)) return null;
  const options = isRecord(widget.options) ? widget.options : {};
  const precision = finite(options.precision);
  const readValue = (): number | null => finite(numericProps.modelValue);
  return {
    name: widget.name,
    label: typeof widget.label === 'string' ? widget.label : widget.name,
    precision,
    minimum: finite(numericProps.min) ?? finite(options.min) ?? 0,
    maximum: finite(numericProps.max) ?? finite(options.max) ?? 1,
    step:
      finite(numericProps.step) ??
      finite(options.step2) ??
      (precision !== null ? 10 ** -precision : 0.01),
    row,
    container,
    spinner,
    input,
    increment: spinner.querySelector<HTMLButtonElement>(
      '[data-testid="increment"], .p-inputnumber-increment-button',
    ),
    decrement: spinner.querySelector<HTMLButtonElement>(
      '[data-testid="decrement"], .p-inputnumber-decrement-button',
    ),
    rangeFill: spinner.querySelector<HTMLElement>(
      ':scope > .pointer-events-none.absolute.size-full',
    ),
    hasNativeSlider: findVueComponents(widgetVNode, 'WidgetInputNumberSlider').length > 0,
    readValue,
    disabled: () =>
      input.disabled ||
      input.readOnly ||
      numericProps.disabled === true ||
      numericProps.readonly === true,
    writeValue: (value) => emit.call(numeric, 'update:modelValue', value),
    subscribe: (listener) => {
      const stop: unknown = watch.call(
        proxy,
        () => [readValue(), numericProps.disabled, numericProps.readonly],
        listener,
        { flush: 'post' },
      );
      if (typeof stop !== 'function')
        throw new TypeError('Comfy numeric value watcher returned an invalid disposer.');
      return () => stop();
    },
  };
}

/** Narrow host numeric properties before using them as presentation geometry or values. */
function finite(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}
