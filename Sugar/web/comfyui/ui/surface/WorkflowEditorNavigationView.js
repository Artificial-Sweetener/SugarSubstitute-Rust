import { createCubeModelTitleElement } from './CubeModelPillDomRenderer.js';
import { createButton } from './appearance/ControlFactory.js';
import { subscribeVisualStyle } from './appearance/VisualStyleBoundary.js';
import { applyOwnedTheme } from './appearance/ThemeService.js';
/** Retain back controls and breadcrumb identity through unrelated native surface updates. */
export class WorkflowEditorNavigationView {
    document;
    element;
    #trail;
    #signature = '';
    #items = [];
    #dark = null;
    #stopStyle;
    /** Bind provider-owned actions to the host's existing navigation commands. */
    constructor(document, back) {
        this.document = document;
        this.element = document.createElement('nav');
        this.element.className = 'sc-editor-navigation';
        this.element.setAttribute('aria-label', 'Workflow editor navigation');
        this.element.setAttribute('data-sc-style-theme-root', '');
        const backButton = this.#back(back);
        backButton.classList.add('sc-editor-bar-back');
        this.#trail = document.createElement('ol');
        this.element.append(backButton, this.#trail);
        this.#stopStyle = subscribeVisualStyle(document, () => {
            const replacement = this.#back(back);
            replacement.classList.add('sc-editor-bar-back');
            this.element.querySelector('.sc-editor-bar-back')?.replaceWith(replacement);
            this.#signature = '';
            const dark = this.#dark ?? true;
            this.#dark = null;
            this.update(this.#items, dark);
        });
    }
    /** Reconcile only changed labels and delegate selection through the latest native path. */
    update(items, dark) {
        this.#items = items;
        if (this.#dark !== dark) {
            this.#dark = dark;
            applyOwnedTheme(this.element, dark);
        }
        const signature = JSON.stringify(items.map(({ label, current, modelTitle }) => [label, current, modelTitle]));
        if (signature === this.#signature)
            return;
        this.#signature = signature;
        this.#trail.replaceChildren(...items.map((item, index) => {
            const entry = this.document.createElement('li');
            const control = item.current
                ? this.document.createElement('span')
                : createButton(this.document);
            if (item.modelTitle)
                control.append(createCubeModelTitleElement(this.document, item.modelTitle));
            else
                control.textContent = item.label;
            control.title = item.label;
            if (item.current)
                control.setAttribute('aria-current', 'page');
            else {
                control.setAttribute('appearance', 'subtle');
                control.addEventListener('click', () => this.#items[index]?.select());
            }
            entry.append(control);
            return entry;
        }));
    }
    /** Detach the trail and forget borrowed breadcrumb entries until the next nested path. */
    clear() {
        this.element.remove();
        this.#items = [];
        this.#signature = '';
        this.#trail.replaceChildren();
    }
    /** Release owned controls without touching native navigation state. */
    dispose() {
        this.#stopStyle();
        this.clear();
    }
    /** Keep the parent action alongside the breadcrumb trail with provider-owned keyboard and focus behavior. */
    #back(action) {
        const button = createButton(this.document);
        button.setAttribute('appearance', 'subtle');
        button.setAttribute('aria-label', 'Back to parent graph');
        button.title = 'Back to parent graph';
        const icon = this.document.createElement('span');
        icon.className = 'icon-[lucide--arrow-left]';
        icon.setAttribute('aria-hidden', 'true');
        button.append(icon);
        button.addEventListener('click', action);
        return button;
    }
}
