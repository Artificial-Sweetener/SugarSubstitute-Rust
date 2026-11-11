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
/** Edit workflow-local source choices through the shared SugarCubes modal lifecycle. */
import { ModalShell } from '../dialogs/ModalShell.js';
/** Keep labels literal and present unsupported savers without offering unsafe interception. */
export class SourceSavingDialog {
    document;
    #shell;
    /** Bind browser rendering and feedback independently of graph mutation. */
    constructor(document, adapter) {
        this.document = document;
        this.#shell = new ModalShell({ adapter, dialogClassName: 'sugarcubes-source-saving-dialog' });
    }
    /** Retain one control set until its fenced edit is accepted or canceled. */
    async open(session, preferences) {
        const body = this.document.createElement('div');
        const fields = new Map();
        if (!session.targets.length) {
            const empty = this.document.createElement('p');
            empty.textContent = 'This workflow has no ordinary image output nodes.';
            body.append(empty);
        }
        for (const source of session.targets) {
            const group = this.document.createElement('fieldset');
            group.style.cssText = 'display:grid;gap:8px;margin:12px 0;padding:12px;min-width:0';
            const title = this.document.createElement('legend');
            title.textContent = source.title;
            title.style.cssText = 'max-width:100%;overflow-wrap:anywhere';
            const identity = this.document.createElement('small');
            identity.textContent = source.classType + ' · ' + source.path;
            identity.style.overflowWrap = 'anywhere';
            const status = this.document.createElement('p');
            status.style.margin = '0';
            status.textContent = !source.supported
                ? 'Native saving · this saver has no verified SugarCubes integration.'
                : !preferences.manage_workflow
                    ? 'Native controls are active. These choices apply when workflow management is enabled.'
                    : source.kind === 'preview' && !preferences.save_previews
                        ? 'Completed preview saving is off.'
                        : 'Saving is managed by SugarCubes.';
            const enabled = this.document.createElement('input');
            enabled.type = 'checkbox';
            enabled.checked = source.preference.enabled;
            enabled.disabled = !source.supported;
            const savingLabel = this.document.createElement('label');
            savingLabel.style.cssText = 'display:flex;gap:8px;align-items:center';
            savingLabel.append(enabled, this.document.createTextNode('Save output when managed'));
            enabled.setAttribute('aria-label', source.title + ' (' + source.path + ') · Save output');
            const label = this.document.createElement('input');
            label.type = 'text';
            label.maxLength = 4096;
            label.value = source.preference.label ?? '';
            label.placeholder = source.title;
            label.setAttribute('aria-label', source.title + ' (' + source.path + ') · Output label');
            const labelRow = this.document.createElement('label');
            labelRow.textContent = 'Output label';
            labelRow.append(label);
            label.style.cssText = 'display:block;width:100%';
            group.append(title, identity, status, savingLabel, labelRow);
            body.append(group);
            fields.set(source.path, { enabled, label });
        }
        await this.#shell.open({
            title: 'Workflow outputs',
            description: 'Source choices are saved with this workflow. Labels appear in the canvas and managed filenames; cube saving is controlled from the cube stack.',
            body,
            confirmLabel: 'Save output settings',
            cancelResult: null,
            onConfirm: () => {
                try {
                    const values = new Map();
                    for (const [path, field] of fields)
                        values.set(path, {
                            enabled: field.enabled.checked,
                            label: field.label.value.trim() || null,
                        });
                    session.commit(values);
                    this.#shell.close(true);
                }
                catch (error) {
                    this.#shell.setError(error instanceof Error ? error.message : 'Output source settings could not be saved.');
                }
            },
        });
    }
}
