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
 * The full-window editor for wildcard files and autocomplete lists, laid out as SugarSubstitute's
 * Python editor was: the files on the left, the selected one in the real prompt editor on the
 * right. Edits are kept per file while others are opened; Save writes one file, Apply writes them
 * all and closes. Closing with unsaved edits asks first.
 */
import { ModalShell } from '../../dialogs/ModalShell.js';
import { ContextMenu } from '../../menus/ContextMenu.js';
import { createFluentIcon } from '../../core/FluentIcons.js';
import { TextAssetDrafts } from '../application/TextAssetDrafts.js';
import { renderTextAssetList } from './TextAssetList.js';
import { installTextAssetStyles } from './TextAssetStyles.js';
/**
 * One file editor, opened as often as its settings row asks. Each opening starts from the files
 * as they are now, with no drafts; the dialog itself is made once and kept.
 */
export class TextAssetEditorModal {
    #options;
    #shell;
    #drafts = new TextAssetDrafts();
    #menu;
    #list;
    #title;
    #save;
    #revert;
    #editorHost;
    #assets = [];
    #selected = null;
    #editor = null;
    /** Increases with every opened file, so a slow mount of an earlier one is dropped. */
    #mounting = 0;
    constructor(options) {
        const document = options.document;
        installTextAssetStyles(document);
        this.#options = options;
        this.#menu = new ContextMenu(document);
        this.#shell = new ModalShell({
            adapter: { getDocument: () => document, getWindow: () => document.defaultView },
            variantClassName: 'sc-text-assets-overlay',
            dialogClassName: 'sc-text-assets-dialog',
            size: 'full-window',
        });
        this.#list = element(document, 'div', 'sc-text-assets__list');
        this.#list.setAttribute('role', 'listbox');
        this.#list.setAttribute('aria-label', options.source.listTitle);
        this.#title = element(document, 'h3', 'sc-text-assets__title');
        this.#save = iconButton(document, 'save', 'Save changes', () => void this.#saveSelected());
        this.#revert = iconButton(document, 'dismiss', 'Discard changes', () => this.#revertSelected());
        this.#editorHost = element(document, 'div', 'sc-text-assets__editor');
    }
    /** Show the editor over its files; resolves when it closes. */
    async open() {
        const document = this.#options.document;
        this.#drafts = new TextAssetDrafts();
        this.#assets = [];
        this.#selected = null;
        const toolbar = element(document, 'div', 'sc-text-assets__toolbar');
        for (const creation of this.#options.source.creations)
            toolbar.append(textButton(document, creation.label, 'add', () => void this.#create(creation)));
        const listCard = element(document, 'section', 'sc-text-assets__card sc-text-assets__files');
        const listTitle = element(document, 'h3', 'sc-text-assets__title');
        listTitle.textContent = this.#options.source.listTitle;
        listCard.append(listTitle, this.#list);
        const editorHead = element(document, 'div', 'sc-text-assets__editor-head');
        editorHead.append(this.#title, this.#save, this.#revert);
        const editorCard = element(document, 'section', 'sc-text-assets__card sc-text-assets__file');
        editorCard.append(editorHead, this.#editorHost);
        const panes = element(document, 'div', 'sc-text-assets__panes');
        panes.append(listCard, editorCard);
        const body = element(document, 'div', 'sc-text-assets');
        body.append(toolbar, panes);
        const closed = this.#shell.open({
            title: this.#options.source.title,
            body,
            confirmLabel: 'Apply',
            cancelLabel: 'Discard',
            confirmClassName: 'p-button-primary',
            allowOverlayClose: false,
            onConfirm: () => void this.#apply(),
            beforeCancel: () => this.#mayDiscard(),
        });
        this.#refreshButtons();
        await this.#reload(null);
        await closed;
        this.#menu.close();
        this.#editor?.dispose();
        this.#editor = null;
    }
    /** Read the files again and select `id`, the selection kept, or the first file. */
    async #reload(id) {
        try {
            this.#assets = await this.#options.source.assets();
        }
        catch (error) {
            this.#fail('Unable to load files', error);
            return;
        }
        const wanted = id ?? this.#selected;
        const next = this.#assets.find((asset) => asset.id === wanted) ?? this.#assets[0] ?? null;
        this.#renderList(next?.id ?? null);
        if (next?.id !== this.#selected || !this.#editor)
            await this.#select(next);
        else
            this.#refreshButtons();
    }
    #renderList(selected) {
        renderTextAssetList(this.#list, this.#assets, selected, this.#options.source.emptyText, {
            select: (asset) => void this.#select(asset),
            menu: (asset, x, y) => this.#openMenu(asset, x, y),
        });
    }
    /** Show `asset` in the editor, reading it the first time it is opened. */
    async #select(asset) {
        if (asset && asset.id === this.#selected && this.#editor)
            return;
        const generation = ++this.#mounting;
        this.#selected = asset?.id ?? null;
        this.#renderList(this.#selected);
        this.#editor?.dispose();
        this.#editor = null;
        this.#editorHost.replaceChildren();
        this.#title.textContent = asset?.label ?? 'No selection';
        this.#title.title = asset?.label ?? '';
        this.#refreshButtons();
        if (!asset)
            return;
        if (!this.#drafts.has(asset.id)) {
            let text;
            try {
                text = await this.#options.source.read(asset.id);
            }
            catch (error) {
                this.#fail('Unable to read file', error);
                return;
            }
            if (text === null) {
                this.#fail('Unable to read file', new Error(`${asset.label} is no longer in the folder.`));
                return;
            }
            this.#drafts.open(asset.id, text);
        }
        if (generation !== this.#mounting)
            return;
        const editor = await this.#options.mountEditor(this.#editorHost, {
            source: this.#drafts.text(asset.id),
            label: asset.label,
            document: asset.document,
            changed: (text) => {
                this.#drafts.edit(asset.id, text);
                this.#refreshButtons();
            },
        });
        if (generation !== this.#mounting) {
            editor.dispose();
            return;
        }
        this.#editor = editor;
    }
    #refreshButtons() {
        const dirty = this.#selected !== null && this.#drafts.dirty(this.#selected);
        this.#save.disabled = !dirty;
        this.#revert.disabled = !dirty;
        this.#shell.setConfirmEnabled(this.#drafts.dirtyIds().length > 0);
    }
    async #saveSelected() {
        const id = this.#selected;
        if (id === null || !this.#drafts.dirty(id))
            return;
        try {
            await this.#options.source.write(id, this.#drafts.text(id));
        }
        catch (error) {
            this.#fail('Unable to save file', error);
            return;
        }
        this.#drafts.saved(id);
        this.#shell.setError('');
        await this.#reload(id);
    }
    #revertSelected() {
        const id = this.#selected;
        if (id === null)
            return;
        this.#drafts.revert(id);
        const asset = this.#assets.find((entry) => entry.id === id) ?? null;
        this.#editor?.dispose();
        this.#editor = null;
        void this.#select(asset);
    }
    /** Write every changed file, tell what follows them, and close. */
    async #apply() {
        this.#shell.setBusy(true);
        try {
            for (const id of this.#drafts.dirtyIds()) {
                await this.#options.source.write(id, this.#drafts.text(id));
                this.#drafts.saved(id);
            }
            await this.#options.source.applied();
        }
        catch (error) {
            this.#shell.setBusy(false);
            this.#fail('Unable to apply changes', error);
            return;
        }
        this.#shell.setBusy(false);
        this.#shell.close(true);
    }
    /** Whether closing may discard what is not saved: nothing, or the person agrees. */
    async #mayDiscard() {
        const unsaved = this.#drafts.dirtyIds().length;
        if (!unsaved)
            return true;
        return this.#options.confirm('Discard unsaved changes?', unsaved === 1
            ? 'One file has changes that are not saved. Closing discards them.'
            : `${String(unsaved)} files have changes that are not saved. Closing discards them.`, 'Discard');
    }
    async #create(creation) {
        const name = await this.#options.prompt({
            title: creation.label,
            label: 'Name',
            confirmLabel: 'Create',
            validate: (value) => (value.trim() ? creation.refusal(value.trim()) : null),
        });
        if (name === null || !name.trim())
            return;
        try {
            await this.#reload(await creation.create(name.trim()));
        }
        catch (error) {
            this.#fail('Unable to create file', error);
        }
    }
    #openMenu(asset, x, y) {
        const source = this.#options.source;
        const entries = [];
        const setEnabled = source.setEnabled?.bind(source);
        if (asset.enabled !== null && setEnabled)
            entries.push({
                label: asset.enabled ? 'Disable' : 'Enable',
                select: () => void this.#run('Unable to update file', asset.id, () => setEnabled(asset, !asset.enabled)),
            });
        entries.push({ label: 'Rename', select: () => void this.#rename(asset) }, { label: 'Delete', select: () => void this.#delete(asset) });
        this.#menu.open({ label: asset.label, entries, anchor: { kind: 'pointer', x, y } });
    }
    async #rename(asset) {
        const source = this.#options.source;
        const name = await this.#options.prompt({
            title: 'Rename',
            label: 'Name',
            confirmLabel: 'Rename',
            initialValue: source.renameText(asset),
            validate: (value) => (value.trim() ? source.renameRefusal(asset, value.trim()) : null),
        });
        if (name === null || !name.trim() || name.trim() === source.renameText(asset))
            return;
        try {
            const id = await source.rename(asset, name.trim());
            this.#drafts.rename(asset.id, id);
            if (this.#selected === asset.id)
                this.#selected = id;
            await this.#reload(id);
        }
        catch (error) {
            this.#fail('Unable to rename file', error);
        }
    }
    async #delete(asset) {
        if (!(await this.#options.confirm('Delete', `Delete '${asset.label}'?`, 'Delete')))
            return;
        await this.#run('Unable to delete file', null, async () => {
            await this.#options.source.delete(asset);
            this.#drafts.drop(asset.id);
            if (this.#selected === asset.id)
                this.#selected = null;
        });
    }
    /** Run one file action, then read the files again and select `id`. */
    async #run(failure, id, action) {
        try {
            await action();
        }
        catch (error) {
            this.#fail(failure, error);
            return;
        }
        this.#shell.setError('');
        await this.#reload(id);
    }
    #fail(title, error) {
        this.#shell.setError(`${title}: ${error instanceof Error ? error.message : String(error)}`);
    }
}
function element(document, tag, className) {
    const created = document.createElement(tag);
    created.className = className;
    return created;
}
function textButton(document, label, icon, run) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'p-button p-component p-button-secondary sc-text-assets__create';
    button.append(createFluentIcon(document, icon), document.createTextNode(label));
    button.addEventListener('click', run);
    return button;
}
function iconButton(document, icon, label, run) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'p-button p-component p-button-text sc-text-assets__tool';
    button.title = label;
    button.setAttribute('aria-label', label);
    button.append(createFluentIcon(document, icon));
    button.addEventListener('click', run);
    return button;
}
