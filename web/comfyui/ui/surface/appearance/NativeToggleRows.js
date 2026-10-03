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
/** Present native boolean state and reversible whole-row interaction. */
/** Delegate row gestures to native switches without replacing their value ownership. */
export class NativeToggleRows {
    #bindings = new Map();
    /** Reconcile only positively identified native toggle fields. */
    reconcile(root) {
        for (const [row, binding] of this.#bindings) {
            if (!row.isConnected ||
                (root.contains(row) && !row.matches('[data-sc-fluent-widget="WidgetToggleSwitch"]'))) {
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
            const stateLabel = row.ownerDocument.createElement('span');
            stateLabel.dataset.scToggleState = '';
            const refresh = () => {
                const input = nativeSwitch(row);
                const switchRoot = input?.closest('[data-pc-name="toggleswitch"]') ?? input;
                if (!input || !switchRoot?.parentElement) {
                    stateLabel.remove();
                    return;
                }
                if (stateLabel.nextSibling !== switchRoot)
                    switchRoot.parentElement.insertBefore(stateLabel, switchRoot);
                const text = input.checked ? 'Enabled' : 'Disabled';
                if (stateLabel.textContent !== text)
                    stateLabel.textContent = text;
            };
            const activate = () => {
                const input = nativeSwitch(row);
                if (input && !input.disabled)
                    input.click();
            };
            const click = (event) => {
                if (!(event.target instanceof Element) ||
                    event.target.closest('input, button, a, label, [role="button"], [data-testid="slot-connection-dot"], [role="switch"], [contenteditable="true"]'))
                    return;
                activate();
                event.stopPropagation();
            };
            const key = (event) => {
                if (event.target !== row || !['Enter', ' '].includes(event.key))
                    return;
                event.preventDefault();
                event.stopPropagation();
                activate();
            };
            const tabindex = row.getAttribute('tabindex');
            row.tabIndex = 0;
            row.dataset.scToggleRow = '';
            row.addEventListener('click', click);
            row.addEventListener('keydown', key);
            row.addEventListener('change', refresh, true);
            const stateObserver = new MutationObserver(refresh);
            stateObserver.observe(row, {
                attributes: true,
                subtree: true,
                attributeFilter: ['aria-checked', 'checked'],
            });
            refresh();
            this.#bindings.set(row, {
                refresh,
                release: () => {
                    row.removeEventListener('click', click);
                    row.removeEventListener('keydown', key);
                    row.removeEventListener('change', refresh, true);
                    stateObserver.disconnect();
                    stateLabel.remove();
                    row.removeAttribute('data-sc-toggle-row');
                    if (tabindex === null)
                        row.removeAttribute('tabindex');
                    else
                        row.setAttribute('tabindex', tabindex);
                },
            });
        }
    }
    /** Restore host row behavior when the theme ends. */
    clear() {
        for (const binding of this.#bindings.values())
            binding.release();
        this.#bindings.clear();
    }
}
/** Read the host-owned boolean value without claiming custom component inputs. */
function nativeSwitch(row) {
    return row.querySelector('input[type="checkbox"], input[role="switch"]');
}
