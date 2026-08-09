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
/** Own complete browser lifecycle and measurement for Cube prompt textareas. */

export type CubePromptTextareaHeightListener = (height: number) => void;

interface PromptTextareaStyleSnapshot {
  height: string;
  heightPriority: string;
  overflowY: string;
  overflowYPriority: string;
}

interface PromptTextareaBinding {
  textarea: HTMLTextAreaElement;
  onHeightChange: CubePromptTextareaHeightListener;
  onInput: () => void;
  originalStyle: PromptTextareaStyleSnapshot;
  originalValueProperty: PropertyDescriptor | undefined;
  valueAccessors: PromptTextareaValueAccessors;
  lastHeight: number | null;
}

interface PromptTextareaValueAccessors {
  enumerable: boolean;
  get: () => string;
  set: (value: string) => void;
}

/** Autosize bound prompt textareas after hydration, input, and width changes. */
export class CubePromptTextareaAutosizer {
  readonly #bindings = new Map<Element, PromptTextareaBinding>();
  readonly #resizeObserver: ResizeObserver | null;

  /** Bind browser scheduling and observation to one active document. */
  constructor(documentRef: Document) {
    const windowRef = documentRef.defaultView;
    if (!windowRef) throw new Error('Prompt textarea sizing requires a browser window.');
    this.#resizeObserver =
      typeof windowRef.ResizeObserver === 'function'
        ? new windowRef.ResizeObserver((entries) => this.#handleResize(entries))
        : null;
  }

  /** Own one textarea until its renderer or DOM-widget adapter releases it. */
  bind(
    textarea: HTMLTextAreaElement,
    onHeightChange: CubePromptTextareaHeightListener = () => undefined,
  ): void {
    const existing = this.#bindings.get(textarea);
    if (existing) {
      existing.onHeightChange = onHeightChange;
      this.#fit(existing);
      return;
    }
    const binding: PromptTextareaBinding = {
      textarea,
      onHeightChange,
      onInput: () => this.#fit(binding),
      originalStyle: capturePromptTextareaStyle(textarea.style),
      originalValueProperty: Object.getOwnPropertyDescriptor(textarea, 'value'),
      valueAccessors: resolvePromptTextareaValueAccessors(textarea),
      lastHeight: null,
    };
    this.#observeProgrammaticValueChanges(binding);
    this.#bindings.set(textarea, binding);
    textarea.addEventListener('input', binding.onInput);
    this.#resizeObserver?.observe(textarea);
    this.#fit(binding);
  }

  /** Restore one textarea exactly and stop every browser callback it owns. */
  unbind(textarea: HTMLTextAreaElement): void {
    const binding = this.#bindings.get(textarea);
    if (!binding) return;
    this.#bindings.delete(textarea);
    textarea.removeEventListener('input', binding.onInput);
    this.#resizeObserver?.unobserve(textarea);
    restorePromptTextareaValueProperty(textarea, binding.originalValueProperty);
    restorePromptTextareaStyle(textarea.style, binding.originalStyle);
  }

  /** Restore every bound textarea and release shared observation. */
  dispose(): void {
    for (const binding of [...this.#bindings.values()]) this.unbind(binding.textarea);
    this.#resizeObserver?.disconnect();
  }

  /** Measure after Comfy changes a bound textarea's rendered width. */
  #handleResize(entries: readonly ResizeObserverEntry[]): void {
    for (const entry of entries) {
      const binding = this.#bindings.get(entry.target);
      if (binding) this.#fit(binding);
    }
  }

  /** Observe Comfy and Vue assigning hydrated values without browser input events. */
  #observeProgrammaticValueChanges(binding: PromptTextareaBinding): void {
    const { textarea, valueAccessors } = binding;
    Object.defineProperty(textarea, 'value', {
      configurable: true,
      enumerable: valueAccessors.enumerable,
      get: valueAccessors.get,
      set: (value: string) => {
        valueAccessors.set(value);
        this.#fit(binding);
      },
    });
  }

  /** Apply the complete content height and report only material changes. */
  #fit(binding: PromptTextareaBinding): void {
    const { textarea } = binding;
    textarea.style.setProperty('height', 'auto', 'important');
    textarea.style.setProperty('overflow-y', 'hidden', 'important');
    const height = Math.max(textarea.scrollHeight, textarea.clientHeight, textarea.offsetHeight, 1);
    textarea.style.setProperty('height', `${String(height)}px`, 'important');
    if (binding.lastHeight !== null && Math.abs(binding.lastHeight - height) < 0.5) return;
    binding.lastHeight = height;
    binding.onHeightChange(height);
  }
}

/** Capture the exact inline properties owned while a prompt is bound. */
function capturePromptTextareaStyle(style: CSSStyleDeclaration): PromptTextareaStyleSnapshot {
  return {
    height: style.height,
    heightPriority: style.getPropertyPriority('height'),
    overflowY: style.overflowY,
    overflowYPriority: style.getPropertyPriority('overflow-y'),
  };
}

/** Resolve the native value accessors so observation preserves textarea semantics. */
function resolvePromptTextareaValueAccessors(
  textarea: HTMLTextAreaElement,
): PromptTextareaValueAccessors {
  let owner: object | null = textarea;
  while (owner) {
    const descriptor = Object.getOwnPropertyDescriptor(owner, 'value');
    if (typeof descriptor?.get === 'function' && typeof descriptor.set === 'function') {
      return {
        enumerable: descriptor.enumerable ?? false,
        get: () => {
          const value: unknown = descriptor.get?.call(textarea);
          return typeof value === 'string' ? value : String(value ?? '');
        },
        set: (value: string) => descriptor.set?.call(textarea, value),
      };
    }
    owner = Reflect.getPrototypeOf(owner);
  }
  throw new Error('Prompt textarea sizing requires native value accessors.');
}

/** Restore the exact value-property ownership that existed before binding. */
function restorePromptTextareaValueProperty(
  textarea: HTMLTextAreaElement,
  descriptor: PropertyDescriptor | undefined,
): void {
  if (descriptor) Object.defineProperty(textarea, 'value', descriptor);
  else Reflect.deleteProperty(textarea, 'value');
}

/** Restore the original inline style without leaving blank owned declarations. */
function restorePromptTextareaStyle(
  style: CSSStyleDeclaration,
  snapshot: PromptTextareaStyleSnapshot,
): void {
  restoreStyleProperty(style, 'height', snapshot.height, snapshot.heightPriority);
  restoreStyleProperty(style, 'overflow-y', snapshot.overflowY, snapshot.overflowYPriority);
}

/** Restore one exact inline property and priority. */
function restoreStyleProperty(
  style: CSSStyleDeclaration,
  property: string,
  value: string,
  priority: string,
): void {
  if (value) style.setProperty(property, value, priority);
  else style.removeProperty(property);
}
