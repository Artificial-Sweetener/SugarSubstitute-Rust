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
/** Expose server-owned update consent in SugarCubes settings without a competing browser preference. */
import { isRecord } from '../types/common.js';
/** Render durable opt-in and explicit checks through the shared update application boundary. */
export class ModelUpdateSettings {
    #updates;
    #document;
    #report;
    #registered = false;
    /** Bind settings presentation independently of focus and HTTP mechanics. */
    constructor(document, updates, report) {
        this.#document = document;
        this.#updates = updates;
        this.#report = report;
    }
    /** Register one custom row that reads and changes the authoritative server preference. */
    setup(app) {
        if (this.#registered)
            return;
        const host = isRecord(app) ? app : {};
        const ui = isRecord(host.ui) ? host.ui : {};
        const settings = isRecord(ui.settings) ? ui.settings : {};
        if (typeof settings.addSetting !== 'function')
            throw new Error('Comfy model update settings are unavailable.');
        this.#registered = true;
        let row = null;
        settings.addSetting.call(settings, {
            id: 'SugarCubes.Models.UpdateNotifications',
            name: 'Model update notifications',
            category: ['SugarCubes', 'Models', 'UpdateNotifications'],
            defaultValue: '',
            tooltip: 'Show compatible updates checked during metadata lookup, daily model selection, and version browsing.',
            type: () => {
                if (row)
                    return row;
                row = this.#document.createElement('div');
                row.className = 'sugarcubes-settings-row';
                const label = this.#document.createElement('label');
                const checkbox = this.#document.createElement('input');
                checkbox.type = 'checkbox';
                checkbox.setAttribute('aria-label', 'Enable model update notifications');
                checkbox.disabled = true;
                label.append(checkbox, this.#document.createTextNode('Enabled'));
                const check = this.#document.createElement('button');
                check.type = 'button';
                check.className = 'p-button p-button-secondary';
                check.textContent = 'Check for updates';
                check.disabled = true;
                row.append(label, check);
                /** Project server consent after every decision, never writing an initialization default. */
                const show = () => {
                    checkbox.checked = this.#updates.read().enabled;
                    check.disabled = !checkbox.checked;
                };
                this.#updates.subscribe(show);
                show();
                void this.#updates
                    .load()
                    .then(() => {
                    show();
                    checkbox.disabled = false;
                })
                    .catch(this.#report);
                checkbox.addEventListener('change', () => {
                    checkbox.disabled = true;
                    void this.#updates
                        .setEnabled(checkbox.checked)
                        .then(async () => {
                        if (this.#updates.read().enabled)
                            await this.#updates.check();
                    })
                        .catch(this.#report)
                        .finally(() => {
                        show();
                        checkbox.disabled = false;
                    });
                });
                check.addEventListener('click', () => {
                    check.disabled = true;
                    void this.#updates.check().catch(this.#report).finally(show);
                });
                return row;
            },
        });
    }
}
