import { ContextMenu } from '../../menus/ContextMenu.js';
const modeLabels = {
    fixed: 'Use Current',
    randomize: 'Randomize',
    increment: 'Increment',
    decrement: 'Decrement',
};
const modeIcons = {
    fixed: 'locked',
    randomize: 'game-die',
    increment: 'add',
    decrement: 'subtract',
};
/** Resolve the pinned Microsoft asset for each supported seed operation. */
function seedIconImage(mode) {
    const url = new URL(`../../../../icons/seed/${modeIcons[mode]}.svg`, import.meta.url);
    return `url("${url.href}")`;
}
/** Own seed accessories separately from native numeric entry and general spinner styling. */
export class NativeSeedControls {
    #cards = new Map();
    /** Retain controls across native redraws and release replaced inputs and detached cards. */
    reconcile(root, fields) {
        for (const [card, bindings] of this.#cards) {
            if (!card.isConnected) {
                for (const binding of bindings.values())
                    binding.release();
                this.#cards.delete(card);
            }
        }
        const bindings = this.#cards.get(root) ?? new Map();
        this.#cards.set(root, bindings);
        for (const [input, binding] of bindings) {
            if (!fields.some(({ numeric }) => numeric.input === input && numeric.spinner === binding.field.numeric.spinner)) {
                binding.release();
                bindings.delete(input);
            }
        }
        for (const field of fields) {
            const existing = bindings.get(field.numeric.input);
            if (existing) {
                existing.field = field;
                existing.synchronize();
            }
            else
                bindings.set(field.numeric.input, this.#bind(field));
        }
    }
    /** Restore native controls without changing any seed value or mode. */
    clear() {
        for (const bindings of this.#cards.values())
            for (const binding of bindings.values())
                binding.release();
        this.#cards.clear();
    }
    /** Attach an accessible quick-toggle button and a separate supported-mode menu. */
    #bind(field) {
        const { spinner } = field.numeric;
        const documentRef = spinner.ownerDocument;
        const split = documentRef.createElement('span');
        split.dataset.scSeedSplit = '';
        const primary = documentRef.createElement('button');
        primary.type = 'button';
        primary.dataset.scSeedToggle = '';
        const icon = documentRef.createElement('span');
        icon.setAttribute('aria-hidden', 'true');
        icon.dataset.scSeedIcon = '';
        primary.append(icon);
        const menuButton = documentRef.createElement('button');
        menuButton.type = 'button';
        menuButton.dataset.scSeedMenu = '';
        menuButton.setAttribute('aria-label', `${field.numeric.label} options`);
        menuButton.setAttribute('aria-haspopup', 'menu');
        menuButton.setAttribute('aria-expanded', 'false');
        const chevron = documentRef.createElement('span');
        chevron.setAttribute('aria-hidden', 'true');
        chevron.className = 'icon-[lucide--chevron-down]';
        menuButton.append(chevron);
        const menu = new ContextMenu(documentRef);
        split.append(primary, menuButton);
        spinner.dataset.scSeedBox = '';
        let renderedMode = '';
        let renderedModes = '';
        const close = () => {
            menu.close();
            menuButton.setAttribute('aria-expanded', 'false');
        };
        const choose = (mode) => {
            if (binding.field.numeric.disabled())
                return;
            binding.field.control.writeMode(mode);
            binding.synchronize();
            close();
            primary.focus();
        };
        const toggle = () => choose(binding.field.control.readMode() === 'randomize' ? 'fixed' : 'randomize');
        const expand = () => {
            if (binding.field.numeric.disabled())
                return;
            if (menu.isOpen()) {
                close();
                return;
            }
            menuButton.setAttribute('aria-expanded', 'true');
            const control = binding.field.control;
            menu.open({
                label: `${binding.field.numeric.label} options`,
                anchor: { kind: 'below', bounds: spinner.getBoundingClientRect() },
                trigger: menuButton,
                focusChecked: true,
                attributes: { 'data-sc-seed-options': '' },
                closed: () => menuButton.setAttribute('aria-expanded', 'false'),
                entries: control.modes.map((mode) => ({
                    label: modeLabels[mode],
                    checked: control.readMode() === mode,
                    attributes: { 'data-sc-seed-option': mode },
                    icon: (document) => {
                        const icon = document.createElement('span');
                        icon.dataset.scSeedIcon = '';
                        icon.style.cssText = `background:currentColor;mask:${seedIconImage(mode)} center/contain no-repeat;`;
                        icon.style.setProperty('--sc-seed-icon', seedIconImage(mode));
                        return icon;
                    },
                    select: () => choose(mode),
                })),
            });
        };
        const binding = {
            field,
            synchronize: () => {
                const control = binding.field.control;
                const mode = control.readMode();
                const disabled = binding.field.numeric.disabled();
                primary.disabled = disabled;
                menuButton.disabled = disabled;
                primary.setAttribute('aria-label', `${binding.field.numeric.label}: ${modeLabels[mode]}; switch to ${mode === 'randomize' ? 'fixed' : 'randomize'}`);
                primary.title = modeLabels[mode];
                primary.dataset.scSeedMode = mode;
                const image = seedIconImage(mode);
                if (icon.style.getPropertyValue('--sc-seed-icon') !== image)
                    icon.style.setProperty('--sc-seed-icon', image);
                if (menu.isOpen() && (mode !== renderedMode || control.modes.join(',') !== renderedModes))
                    close();
                renderedMode = mode;
                renderedModes = control.modes.join(',');
                if (disabled)
                    close();
            },
            release: () => {
                close();
                unsubscribe();
                split.remove();
                delete spinner.dataset.scSeedBox;
                primary.removeEventListener('click', toggle);
                menuButton.removeEventListener('click', expand);
            },
        };
        let unsubscribe = () => undefined;
        primary.addEventListener('click', toggle);
        menuButton.addEventListener('click', expand);
        spinner.append(split);
        try {
            binding.synchronize();
            unsubscribe = field.numeric.subscribe(binding.synchronize);
        }
        catch (error) {
            binding.release();
            throw error;
        }
        return binding;
    }
}
