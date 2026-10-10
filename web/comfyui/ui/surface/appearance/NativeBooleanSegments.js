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
/** Present named native boolean choices with Microsoft's sliding Tablist and host-owned values. */
/** Own reversible segmented presentation without making the surrounding field interactive. */
export class NativeBooleanSegments {
    #bindings = new Map();
    #createControls;
    /** Receive the official Fluent control at the third-party boundary. */
    constructor(createControls) {
        this.#createControls = createControls;
    }
    /** Reconcile identified native choices while retaining mounted button and focus identity. */
    reconcile(root) {
        for (const [row, binding] of this.#bindings) {
            const current = root.contains(row) ? readNativeChoices(row) : binding.native;
            if (!row.isConnected ||
                !binding.native.group.isConnected ||
                current?.group !== binding.native.group ||
                current.buttons.some((button, index) => button !== binding.native.buttons[index])) {
                binding.release();
                this.#bindings.delete(row);
            }
        }
        for (const row of root.querySelectorAll('[data-sc-fluent-row][data-sc-fluent-widget="WidgetToggleSwitch"]')) {
            const existing = this.#bindings.get(row);
            if (existing) {
                existing.refresh();
                continue;
            }
            const native = readNativeChoices(row);
            if (native)
                this.#bindings.set(row, this.#bind(row, native));
        }
    }
    /** Restore the host's current controls when Fluent appearance ends. */
    clear() {
        for (const binding of this.#bindings.values())
            binding.release();
        this.#bindings.clear();
    }
    /** Delegate segment activation to one native edit and observe only that native group's state. */
    #bind(row, native) {
        const controls = this.#createControls(row.ownerDocument);
        const { group, segments } = controls;
        group.dataset.scBooleanSegments = '';
        group.setAttribute('size', 'small');
        group.setAttribute('appearance', 'subtle');
        segments.forEach((segment) => {
            segment.dataset.scBooleanSegment = '';
            segment.slot = 'tab';
            group.append(segment);
        });
        let reflecting = false;
        let released = false;
        const refresh = () => {
            if (released)
                return;
            const label = row.querySelector('[data-testid="widget-layout-field-label"]')?.textContent ??
                native.group.getAttribute('aria-label') ??
                '';
            if (group.getAttribute('aria-label') !== label)
                group.setAttribute('aria-label', label);
            native.buttons.forEach((button, index) => {
                const segment = segments[index];
                if (!segment)
                    return;
                const label = button.textContent ?? '';
                if (segment.textContent !== label)
                    segment.textContent = label;
                const pressed = button.getAttribute('aria-pressed') === 'true';
                const selected = String(pressed);
                if (segment.getAttribute('aria-selected') !== selected)
                    segment.setAttribute('aria-selected', selected);
                if (segment.disabled !== button.disabled)
                    segment.disabled = button.disabled;
            });
            const selected = segments.find((segment) => segment.getAttribute('aria-selected') === 'true');
            if (selected && group.activeid !== selected.id) {
                reflecting = true;
                try {
                    group.activeid = selected.id;
                }
                finally {
                    reflecting = false;
                }
            }
        };
        const activate = (index) => {
            const button = native.buttons[index];
            if (button && !button.disabled && button.getAttribute('aria-pressed') !== 'true')
                button.click();
            // Vue reflects the host edit after its event handler; the adapter retains no boolean state.
            queueMicrotask(refresh);
        };
        const click = (event) => {
            const index = segments.findIndex((segment) => event.composedPath().includes(segment));
            if (index < 0)
                return;
            event.stopPropagation();
        };
        const selection = (event) => {
            event.stopPropagation();
            if (reflecting || released)
                return;
            activate(segments.findIndex((segment) => segment.id === group.activeid));
        };
        const key = (event) => {
            event.stopPropagation();
            if (!['Enter', ' '].includes(event.key))
                return;
            const index = segments.findIndex((segment) => event.composedPath().includes(segment));
            if (index < 0)
                return;
            event.preventDefault();
            const segment = segments[index];
            if (segment && !segment.disabled)
                group.activeid = segment.id;
        };
        const retainGesture = (event) => event.stopPropagation();
        const nativeClick = () => queueMicrotask(refresh);
        group.addEventListener('click', click);
        group.addEventListener('keydown', key);
        group.addEventListener('change', selection);
        group.addEventListener('pointerdown', retainGesture);
        native.group.addEventListener('click', nativeClick);
        const stateObserver = new MutationObserver(refresh);
        stateObserver.observe(native.group, {
            attributes: true,
            subtree: true,
            childList: true,
            characterData: true,
            attributeFilter: ['aria-pressed', 'disabled'],
        });
        refresh();
        native.group.before(group);
        native.group.setAttribute('data-sc-boolean-native', '');
        return {
            native,
            refresh,
            release: () => {
                released = true;
                stateObserver.disconnect();
                group.removeEventListener('click', click);
                group.removeEventListener('keydown', key);
                group.removeEventListener('change', selection);
                group.removeEventListener('pointerdown', retainGesture);
                native.group.removeEventListener('click', nativeClick);
                native.group.removeAttribute('data-sc-boolean-native');
                group.remove();
                controls.release();
            },
        };
    }
}
/** Accept only the host's off/on button pair on a positively identified boolean widget. */
function readNativeChoices(row) {
    if (!row.matches('[data-sc-fluent-row][data-sc-fluent-widget="WidgetToggleSwitch"]'))
        return null;
    for (const group of row.querySelectorAll('[role="group"]')) {
        const buttons = Array.from(group.querySelectorAll(':scope > button'));
        const [off, on] = buttons;
        if (buttons.length === 2 &&
            off?.value === 'off' &&
            on?.value === 'on' &&
            buttons.every((button) => ['true', 'false'].includes(button.getAttribute('aria-pressed') ?? '')))
            return { group, buttons: [off, on] };
    }
    return null;
}
