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
 * Show every product's release notes in one SugarCubes dialog.
 *
 * The dialog is the shared {@link ModalShell}; each product is one labelled section that shows
 * its notes, a loading line, or a plain failure with Retry. Showing again while open replaces
 * the sections in place, so a product whose notes arrive later joins the same dialog.
 */
import { ModalShell } from './ModalShell.js';
import { renderReleaseNotes } from './ReleaseNotesMarkdown.js';
import { WHATS_NEW_TEXT } from './WhatsNewText.js';
/** The What's new dialog. */
export class WhatsNewModal {
    shell;
    documentRef;
    open = false;
    constructor({ adapter, document }) {
        this.documentRef = document;
        this.shell = new ModalShell({
            adapter: adapter ?? { getDocument: () => document, getWindow: () => document.defaultView },
            variantClassName: 'sugarcubes-whats-new-overlay',
            dialogClassName: 'sugarcubes-whats-new-dialog',
        });
    }
    show(sections, actions) {
        const body = sections.map((section) => this.#section(section, actions));
        if (this.open) {
            this.shell.setBody(body);
            return;
        }
        this.open = true;
        void this.shell
            .open({
            title: WHATS_NEW_TEXT.title,
            body,
            confirmLabel: WHATS_NEW_TEXT.close,
            showCancel: false,
            onConfirm: () => this.shell.close(null),
        })
            .then(() => {
            this.open = false;
            actions.closed();
        });
    }
    #section(section, actions) {
        const document = this.documentRef;
        const label = WHATS_NEW_TEXT.section(section.product, section.currentVersion, section.updatedFrom);
        const element = document.createElement('section');
        element.className = 'sugarcubes-whats-new__section';
        element.setAttribute('aria-label', label);
        const heading = document.createElement('h3');
        heading.className = 'sugarcubes-whats-new__product';
        heading.textContent = label;
        element.append(heading);
        const status = section.status;
        if (status.kind === 'loading') {
            element.append(this.#line(WHATS_NEW_TEXT.loading, 'status'));
        }
        else if (status.kind === 'failed') {
            if (status.reason === 'not-published')
                element.append(this.#line(WHATS_NEW_TEXT.notPublished(section.product, section.currentVersion), null));
            else {
                element.append(this.#line(WHATS_NEW_TEXT.failed, 'alert'));
                const retry = document.createElement('button');
                retry.type = 'button';
                retry.className = 'p-button p-component p-button-secondary sugarcubes-whats-new__retry';
                retry.textContent = WHATS_NEW_TEXT.retry;
                retry.addEventListener('click', () => actions.retry(section.key));
                element.append(retry);
            }
        }
        else {
            for (const release of status.releases) {
                const article = document.createElement('article');
                article.className = 'sugarcubes-whats-new__release';
                article.dataset.version = release.version;
                article.append(renderReleaseNotes(document, release.notes));
                element.append(article);
            }
            if (status.omitted > 0)
                element.append(this.#line(WHATS_NEW_TEXT.omitted(status.omitted), null));
        }
        return element;
    }
    #line(text, role) {
        const line = this.documentRef.createElement('p');
        line.className = 'sugarcubes-whats-new__line';
        if (role)
            line.setAttribute('role', role);
        line.textContent = text;
        return line;
    }
}
