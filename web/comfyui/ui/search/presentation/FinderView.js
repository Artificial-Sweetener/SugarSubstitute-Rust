import { applyOwnedTheme } from '../../surface/appearance/ThemeService.js';
import { createFinderInput, createButton } from '../../surface/appearance/ControlFactory.js';
import { readComfyDarkTheme } from '../../surface/appearance/ComfyColorScheme.js';
import { ensureFinderStyles } from './FinderStyles.js';
import { ContextMenu } from '../../menus/ContextMenu.js';
/** Retain selected provider controls while finder state and results change. */
export class FinderView {
    element;
    #input;
    #mode;
    #modeLabel;
    #menu;
    #next;
    #previous;
    #clear;
    #status;
    #actions;
    #context = 'text';
    #syncing = false;
    /** Match the reference's 296px input group, context selector and ordered icon buttons. */
    constructor(documentRef, actions) {
        this.#actions = actions;
        ensureFinderStyles(documentRef);
        this.element = documentRef.createElement('div');
        this.element.className = 'sc-masonry-finder';
        this.element.setAttribute('role', 'search');
        this.element.setAttribute('aria-label', 'Find in masonry');
        this.element.setAttribute('data-capture-wheel', 'true');
        this.element.hidden = true;
        applyOwnedTheme(this.element, readComfyDarkTheme(documentRef));
        this.#input = createFinderInput(documentRef);
        this.#input.className = 'sc-masonry-finder__input';
        this.#input.setAttribute('aria-label', 'Search masonry');
        this.#input.setAttribute('placeholder', 'Search…');
        this.#input.setAttribute('control-size', 'medium');
        this.#menu = new ContextMenu(documentRef);
        this.#mode = createButton(documentRef);
        this.#mode.className = 'sc-masonry-finder__mode';
        this.#mode.setAttribute('role', 'button');
        this.#mode.setAttribute('slot', 'start');
        this.#mode.setAttribute('aria-label', 'Search context');
        this.#mode.setAttribute('appearance', 'subtle');
        this.#mode.setAttribute('aria-haspopup', 'menu');
        this.#mode.setAttribute('aria-expanded', 'false');
        this.#modeLabel = documentRef.createElement('span');
        const modeContent = documentRef.createElement('span');
        modeContent.className = 'sc-masonry-finder__mode-content';
        const chevron = documentRef.createElement('span');
        chevron.className = 'sc-masonry-finder__chevron';
        chevron.setAttribute('aria-hidden', 'true');
        modeContent.append(this.#modeLabel, chevron);
        this.#mode.append(modeContent);
        this.#mode.addEventListener('click', this.#toggleContext);
        this.#clear = this.#button('Clear search', 'dismiss', () => {
            this.#input.value = '';
            this.#change();
            this.#input.focusInput(false);
        });
        const search = this.#button('Search', 'search', () => this.#change());
        const end = documentRef.createElement('span');
        end.className = 'sc-masonry-finder__input-actions';
        end.slot = 'end';
        end.append(this.#clear, search);
        const group = documentRef.createElement('div');
        group.className = 'sc-masonry-finder__group';
        group.append(this.#mode, this.#input, end);
        this.#next = this.#button('Next match (Enter)', 'arrow-down', () => actions.cycle(1));
        this.#previous = this.#button('Previous match (Shift+Enter)', 'arrow-up', () => actions.cycle(-1));
        const close = this.#button('Close search (Esc)', 'dismiss', () => actions.close(false));
        this.#status = documentRef.createElement('span');
        this.#status.className = 'sc-masonry-finder__status';
        this.#status.setAttribute('role', 'status');
        this.#status.setAttribute('aria-live', 'polite');
        this.element.append(group, this.#next, this.#previous, close, this.#status);
        this.#input.addEventListener('input', this.#change);
        this.element.addEventListener('keydown', this.#key);
        this.element.addEventListener('pointerdown', this.#stop);
        this.element.addEventListener('wheel', this.#stop);
    }
    /** Reflect consumed context commands without replacing controls or disturbing normal input selection. */
    sync(mode, text, count, index, status) {
        this.#syncing = true;
        this.#context = mode;
        if (this.#input.value !== text)
            this.#input.value = text;
        this.#modeLabel.textContent = mode === 'text' ? 'Text' : mode === 'field' ? 'Field' : 'Node';
        this.#clear.hidden = !text;
        const enabled = mode !== 'field' && count > 0;
        for (const button of [this.#next, this.#previous])
            button.toggleAttribute('disabled', !enabled);
        this.#status.textContent = count ? `${index + 1} of ${count} matches` : status;
        this.#syncing = false;
    }
    /** Place the overlay in its owning viewport so it stays below native Comfy chrome. */
    show(container, focus) {
        if (this.element.parentElement !== container)
            container.append(this.element);
        this.element.hidden = false;
        applyOwnedTheme(this.element, readComfyDarkTheme(this.element.ownerDocument));
        if (focus) {
            this.#input.focusInput(true);
        }
    }
    /** Retain the query controls while removing the visible overlay. */
    hide() {
        this.#menu.close();
        this.element.hidden = true;
    }
    /** Remove the owned overlay and scoped theme on surface release. */
    dispose() {
        this.#menu.close();
        applyOwnedTheme(this.element, null);
        this.element.remove();
    }
    /** Emit only validated context values from selected provider controls. */
    #change = () => {
        if (this.#syncing)
            return;
        this.#actions.input(this.#context, this.#input.value);
    };
    /** Toggle the shared context menu while retaining its normal keyboard and focus behavior. */
    #toggleContext = () => {
        if (this.#menu.isOpen()) {
            this.#menu.close();
            return;
        }
        const modes = ['text', 'field', 'node'];
        this.#mode.setAttribute('aria-expanded', 'true');
        this.#menu.open({
            label: 'Search context',
            anchor: { kind: 'below', bounds: this.#mode.getBoundingClientRect() },
            trigger: this.#mode,
            focusChecked: true,
            closed: () => this.#mode.setAttribute('aria-expanded', 'false'),
            entries: modes.map((mode) => ({
                label: mode === 'text' ? 'Text' : mode === 'field' ? 'Field' : 'Node',
                checked: mode === this.#context,
                selection: 'single',
                select: () => {
                    this.#actions.input(mode, this.#input.value);
                    this.#input.focusInput(false);
                },
            })),
        });
    };
    /** Match reference keyboard behavior while preventing graph commands inside the finder. */
    #key = (event) => {
        if (event.composedPath().includes(this.#input.control)) {
            if (event.key === 'Escape') {
                event.preventDefault();
                this.#actions.close(true);
            }
            else if (event.key === 'Enter' && this.#context === 'text') {
                event.preventDefault();
                this.#actions.cycle(event.shiftKey ? -1 : 1);
            }
        }
        event.stopPropagation();
    };
    /** Keep finder pointer and wheel input out of graph event handlers. */
    #stop = (event) => event.stopPropagation();
    /** Use selected provider buttons with licensed Microsoft system icons. */
    #button(label, icon, action) {
        const documentRef = this.element.ownerDocument;
        const button = createButton(documentRef);
        button.setAttribute('role', 'button');
        button.setAttribute('appearance', 'subtle');
        button.setAttribute('shape', 'rounded');
        button.setAttribute('icon-only', '');
        button.setAttribute('aria-label', label);
        button.title = label;
        const image = documentRef.createElement('span');
        image.className = 'sc-masonry-finder__icon';
        const url = new URL(`../../../../icons/search/${icon}.svg`, import.meta.url);
        image.style.setProperty('--sc-finder-icon', `url("${url.href}")`);
        image.setAttribute('aria-hidden', 'true');
        button.append(image);
        button.addEventListener('click', action);
        return button;
    }
}
