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
/** Adapt recognized Comfy Vue controls without replacing their value or connection owners. */
import { isRecord } from '../../types/common.js';
import type { UnknownRecord } from '../../types/common.js';
import { findVueComponents } from '../../surface/ComfyVueTree.js';
import type { EditorField } from '../application/EditorField.js';
import type { PresetValue } from '../domain/UserPreset.js';
import { copySavablePresetValue } from '../domain/PresetCodec.js';
import type { NativeAdvancedInputControl } from '../../surface/NativeAdvancedInputControl.js';
import { readNativeEditorModelInputs } from './NativeEditorModelInputs.js';
import type { NativeEditorModelInput } from './NativeEditorModelInputs.js';

export interface NativeEditorField extends EditorField {
  readonly element: HTMLElement;
}
export interface NativeEditorCard {
  readonly root: HTMLElement;
  readonly nodeType: string;
  readonly title: string;
  readonly fields: readonly NativeEditorField[];
  readonly modelInputs: readonly NativeEditorModelInput[];
  readonly header: HTMLElement | null;
  readonly advanced: NativeAdvancedInputControl | null;
}
const components = new Map<string, EditorField['kind']>([
  ['WidgetInputNumber', 'number'],
  ['WidgetInputNumberInput', 'number'],
  ['WidgetInputNumberSlider', 'number'],
  ['WidgetToggleSwitch', 'boolean'],
  ['WidgetSelect', 'combo'],
  ['WidgetSelectDefault', 'combo'],
  ['WidgetInputText', 'string'],
  ['WidgetTextarea', 'string'],
]);

/** Resolve a positively identified node component and only its own standard widget descendants. */
export function readNativeEditorCard(root: HTMLElement): NativeEditorCard | null {
  const mount = projectedEditorCardMount(root);
  if (!mount) return null;
  const record: UnknownRecord = isRecord(mount) ? mount : {};
  const vnode = findVueComponents(record._vnode, 'LGraphNode').find(
    (vnode) => elementOf(vnode) === root,
  );
  const instance = isRecord(vnode?.component) ? vnode.component : {};
  const props = isRecord(instance.props) ? instance.props : {};
  const data = isRecord(props.nodeData) ? props.nodeData : {};
  if (typeof data.type !== 'string') return null;
  const fields: NativeEditorField[] = [];
  const widgetContainers = findVueComponents(vnode, 'NodeWidgets')
    .map(elementOf)
    .filter((element): element is HTMLElement => element?.closest('.lg-node') === root);
  for (const [component, kind] of components) {
    for (const child of findVueComponents(vnode, component)) {
      const field = readField(child, root, kind, widgetContainers);
      if (field && !fields.some((existing) => existing.name === field.name)) fields.push(field);
    }
  }
  const hasOwnedWidgets = Array.isArray(data.sugarcubesEditorWidgets);
  const declared = hasOwnedWidgets ? data.sugarcubesEditorWidgets : data.widgets;
  if (Array.isArray(declared))
    for (const widget of declared) {
      if (!isRecord(widget) || fields.some((field) => field.name === widget.name)) continue;
      const field = readUnrenderedField(widget, root, hasOwnedWidgets);
      if (field) fields.push(field);
    }
  // Preserve authored order regardless of Vue's traversal direction or nested wrapper components.
  const names = Array.isArray(declared)
    ? declared.filter(isRecord).map((widget) => widget.name)
    : [];
  fields.sort((left, right) => names.indexOf(left.name) - names.indexOf(right.name));
  const own = <T extends HTMLElement>(selector: string): T | null =>
    [...root.querySelectorAll<T>(selector)].find(
      (element) => element.closest('.lg-node') === root,
    ) ?? null;
  const header = own('.lg-node-header');
  return {
    root,
    nodeType: data.type,
    title: typeof data.title === 'string' ? data.title : data.type,
    fields,
    modelInputs: readNativeEditorModelInputs(declared, fields),
    header,
    advanced: readAdvancedControl(
      data.sugarcubesAdvancedInputs,
      own('[data-testid="advanced-inputs-button"]'),
    ),
  };
}

/** Restrict editor actions to the shared native-card mounts owned by masonry and Cube faces. */
export function projectedEditorCardMount(root: HTMLElement): HTMLElement | null {
  return root.closest<HTMLElement>(
    '.sugarcubes-cube-face__native-card-mount.sugarcubes-native-node-card',
  );
}

/** Prefer the exact native disclosure owner, retaining Comfy's public button for ordinary graph cards. */
function readAdvancedControl(
  value: unknown,
  button: HTMLButtonElement | null,
): NativeAdvancedInputControl | null {
  if (
    isRecord(value) &&
    typeof value.shown === 'function' &&
    typeof value.setShown === 'function'
  ) {
    const shown = value.shown,
      setShown = value.setShown;
    return {
      shown: () => shown.call(value) === true,
      setShown: (next) => {
        setShown.call(value, next);
      },
    };
  }
  if (!button) return null;
  return {
    shown: () => button.textContent?.includes('Hide') ?? false,
    setShown: (next) => {
      if (next !== (button.textContent?.includes('Hide') ?? false)) button.click();
    },
  };
}

/** Retain the native emitter and re-read dynamic constraints at every command execution. */
function readField(
  vnode: UnknownRecord,
  root: HTMLElement,
  kind: EditorField['kind'],
  containers: readonly HTMLElement[],
): NativeEditorField | null {
  const instance = isRecord(vnode.component) ? vnode.component : {};
  const props = isRecord(instance.props) ? instance.props : {};
  const widget = isRecord(props.widget) ? props.widget : {};
  const element = elementOf(vnode),
    emit = instance.emit;
  if (
    !element ||
    element.closest('.lg-node') !== root ||
    typeof widget.name !== 'string' ||
    widget.name === 'control_after_generate' ||
    typeof emit !== 'function'
  )
    return null;
  const options = (): UnknownRecord => (isRecord(widget.options) ? widget.options : {});
  const disabled = (): boolean => {
    const constraints = options();
    const input = element.querySelector<HTMLInputElement | HTMLTextAreaElement>('input,textarea');
    return (
      !root.isConnected ||
      props.disabled === true ||
      props.readonly === true ||
      constraints.disabled === true ||
      constraints.readonly === true ||
      Boolean(input?.disabled || input?.readOnly)
    );
  };
  const read = (): unknown => props.modelValue;
  return {
    name: widget.name,
    kind,
    element: fieldRow(element, containers) ?? element,
    read,
    disabled,
    write: (value: PresetValue): boolean => {
      if (disabled()) return false;
      const next = normalizeValue(kind, value, options(), widget.spec);
      if (next === undefined) return false;
      emit.call(instance, 'update:modelValue', next);
      // The existing promoted-widget lease observes native edit events after Vue callbacks settle.
      element.dispatchEvent(new Event('change', { bubbles: true }));
      return true;
    },
  };
}

/** Apply declared advanced controls through the same data callback used by Comfy's NodeWidgets. */
function readUnrenderedField(
  widget: UnknownRecord,
  root: HTMLElement,
  owned: boolean,
): NativeEditorField | null {
  if (
    typeof widget.name !== 'string' ||
    widget.name === 'control_after_generate' ||
    (!owned && !Array.isArray(widget.spec) && !isRecord(widget.spec))
  )
    return null;
  const options = (): UnknownRecord => (isRecord(widget.options) ? widget.options : {});
  if (options().canvasOnly === true || widget.serialize === false) return null;
  const kind: EditorField['kind'] | null =
    widget.type === 'number'
      ? 'number'
      : widget.type === 'toggle'
        ? 'boolean'
        : widget.type === 'combo'
          ? 'combo'
          : ['text', 'string', 'customtext'].includes(String(widget.type))
            ? 'string'
            : owned && typeof widget.type === 'string' && typeof widget.callback === 'function'
              ? 'json'
              : null;
  if (!kind) return null;
  const disabled = (): boolean =>
    !root.isConnected ||
    options().disabled === true ||
    options().readonly === true ||
    (isRecord(widget.slotMetadata) && widget.slotMetadata.linked === true);
  return {
    name: widget.name,
    kind,
    element: root,
    read: () => widget.value,
    disabled,
    write: (value) => {
      if (disabled() || typeof widget.callback !== 'function') return false;
      const next = normalizeValue(kind, value, options(), widget.spec);
      if (next === undefined) return false;
      widget.value = next;
      widget.callback(next);
      root.dispatchEvent(new Event('change', { bubbles: true }));
      return true;
    },
  };
}

/** Match native numeric setters' clamping while refusing incompatible or malformed values. */
function normalizeValue(
  kind: EditorField['kind'],
  value: PresetValue,
  options: UnknownRecord,
  spec: unknown,
): PresetValue | undefined {
  if (kind === 'json') return copySavablePresetValue(value);
  if (kind === 'boolean') return typeof value === 'boolean' ? value : undefined;
  if (kind === 'string') return typeof value === 'string' ? value : undefined;
  if (kind === 'combo')
    return typeof value === 'string' &&
      Array.isArray(options.values) &&
      options.values.includes(value)
      ? value
      : undefined;
  if (typeof value !== 'number' || !Number.isFinite(value)) return undefined;
  const integer =
    options.precision === 0 ||
    (isRecord(spec) && spec.type === 'INT') ||
    (Array.isArray(spec) && spec[0] === 'INT');
  if (integer && !Number.isInteger(value)) return undefined;
  const min = typeof options.min === 'number' ? options.min : -Infinity;
  const max = typeof options.max === 'number' ? options.max : Infinity;
  if (Number.isNaN(min) || Number.isNaN(max) || min > max) return undefined;
  const next = Math.min(max, Math.max(min, value));
  return Number.isFinite(next) && (!integer || Number.isInteger(next)) ? next : undefined;
}

/** Include labels and padding only within a positively identified native NodeWidgets row. */
function fieldRow(element: HTMLElement, containers: readonly HTMLElement[]): HTMLElement | null {
  for (const container of containers)
    for (const row of container.children)
      if (row instanceof HTMLElement && row !== element && row.contains(element)) return row;
  return null;
}
/** Narrow Vue's external DOM root before associating any menu action with it. */
function elementOf(vnode: UnknownRecord): HTMLElement | null {
  const instance = isRecord(vnode.component) ? vnode.component : {};
  const tree = isRecord(instance.subTree) ? instance.subTree : {};
  return tree.el instanceof HTMLElement
    ? tree.el
    : vnode.el instanceof HTMLElement
      ? vnode.el
      : null;
}
