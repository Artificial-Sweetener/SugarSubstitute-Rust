import { usesSubstituteSlider } from './NumericSliderPresentation.js';
/** Attach the selected slider beside the existing spinbox only at recognized native boundaries. */
export class NativeNumericControls {
    #cards = new Map();
    #createSlider;
    /** Bind the official component factory independently of native widget adaptation. */
    constructor(createSlider) {
        this.#createSlider = createSlider;
    }
    /** Follow replaced native inputs and reapply current presentation without duplicating controls. */
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
            const replacement = fields.find((field) => field.input === input);
            if (!replacement ||
                replacement.container !== binding.field.container ||
                replacement.spinner !== binding.field.spinner ||
                (usesSubstituteSlider(replacement) && !replacement.hasNativeSlider) !==
                    (binding.slider !== null)) {
                binding.release();
                bindings.delete(input);
            }
        }
        for (const field of fields) {
            const existing = bindings.get(field.input);
            if (existing) {
                existing.field = field;
                existing.synchronize();
            }
            else
                bindings.set(field.input, this.#bind(field));
        }
    }
    /** Release controls, subscriptions, and annotations when the theme or renderer ends. */
    clear() {
        for (const bindings of this.#cards.values())
            for (const binding of bindings.values())
                binding.release();
        this.#cards.clear();
    }
    /** Synchronize through native model events so all edits use Comfy's existing value path. */
    #bind(field) {
        const slider = usesSubstituteSlider(field) && !field.hasNativeSlider
            ? this.#createSlider(field.row.ownerDocument)
            : null;
        const annotations = new Map();
        const gestureTargets = slider ? [field.spinner, slider.root] : [field.spinner];
        for (const target of gestureTargets)
            target.addEventListener('pointerdown', retainNumericGesture);
        let synchronizing = false;
        let unsubscribe = () => undefined;
        const publishSliderValue = () => {
            if (synchronizing || !slider || binding.field.disabled())
                return;
            const value = slider.readValue();
            if (Number.isFinite(value) && value !== binding.field.readValue())
                binding.field.writeValue(value);
        };
        const binding = {
            field,
            slider,
            synchronize: () => {
                synchronizeNumericAnnotations(binding.field, annotations);
                if (!slider)
                    return;
                synchronizing = true;
                try {
                    slider.synchronize(binding.field.minimum, binding.field.maximum, binding.field.step, binding.field.disabled(), binding.field.readValue());
                }
                finally {
                    synchronizing = false;
                }
            },
            release: () => {
                unsubscribe();
                for (const target of gestureTargets)
                    target.removeEventListener('pointerdown', retainNumericGesture);
                for (const [attribute, element] of annotations)
                    element.removeAttribute(attribute);
                annotations.clear();
                field.container.removeAttribute('data-sc-number-pair');
                slider?.root.removeEventListener('input', publishSliderValue);
                slider?.root.removeEventListener('change', publishSliderValue);
                slider?.root.removeEventListener('keydown', retainSliderNavigation);
                slider?.root.remove();
            },
        };
        if (slider) {
            slider.root.setAttribute('aria-label', `${field.label} slider`);
            slider.root.dataset.scNumberSlider = '';
            field.container.setAttribute('data-sc-number-pair', '');
            slider.root.addEventListener('input', publishSliderValue);
            slider.root.addEventListener('change', publishSliderValue);
            slider.root.addEventListener('keydown', retainSliderNavigation);
            field.container.append(slider.root);
        }
        try {
            binding.synchronize();
            if (slider)
                unsubscribe = field.subscribe(binding.synchronize);
        }
        catch (error) {
            binding.release();
            throw error;
        }
        return binding;
    }
}
/** Follow late-mounted native buttons while retaining the numeric input and slider identities. */
function synchronizeNumericAnnotations(field, annotations) {
    const targets = [
        ['data-sc-number-spinner', field.spinner],
        ['data-sc-number-input', field.input],
        ['data-sc-number-increment', field.increment],
        ['data-sc-number-decrement', field.decrement],
        ['data-sc-native-number-fill', field.rangeFill],
    ];
    for (const [attribute, element] of targets) {
        const previous = annotations.get(attribute);
        if (previous === element)
            continue;
        previous?.removeAttribute(attribute);
        annotations.delete(attribute);
        if (element) {
            element.setAttribute(attribute, '');
            annotations.set(attribute, element);
        }
    }
}
/** Keep primary gesture starts away from Comfy's node drag owner; themed retains window move/up. */
function retainNumericGesture(event) {
    if (event instanceof MouseEvent && event.button === 0)
        event.stopPropagation();
}
/** Preserve native range navigation before Comfy's graph shortcuts can cancel its default value change. */
function retainSliderNavigation(event) {
    if ([
        'ArrowLeft',
        'ArrowRight',
        'ArrowUp',
        'ArrowDown',
        'Home',
        'End',
        'PageUp',
        'PageDown',
    ].includes(event.key))
        event.stopPropagation();
}
