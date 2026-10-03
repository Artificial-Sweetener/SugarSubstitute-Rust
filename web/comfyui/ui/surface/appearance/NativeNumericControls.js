import { usesSubstituteSlider } from './NumericSliderPresentation.js';
/** Attach Microsoft's slider beside the existing spinbox only at recognized native boundaries. */
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
        const annotations = [
            [field.spinner, 'data-sc-number-spinner'],
            [field.input, 'data-sc-number-input'],
            [field.increment, 'data-sc-number-increment'],
            [field.decrement, 'data-sc-number-decrement'],
        ];
        for (const [element, attribute] of annotations)
            element?.setAttribute(attribute, '');
        const gestureTargets = slider ? [field.spinner, slider] : [field.spinner];
        for (const target of gestureTargets)
            target.addEventListener('pointerdown', retainNumericGesture);
        let synchronizing = false;
        let unsubscribe = () => undefined;
        const change = () => {
            if (synchronizing || !slider || binding.field.disabled())
                return;
            const value = Number(slider.value);
            if (Number.isFinite(value) && value !== binding.field.readValue())
                binding.field.writeValue(value);
        };
        const binding = {
            field,
            slider,
            synchronize: () => {
                if (!slider)
                    return;
                synchronizing = true;
                try {
                    slider.min = String(binding.field.minimum);
                    slider.max = String(binding.field.maximum);
                    slider.step = String(binding.field.step);
                    slider.disabled = binding.field.disabled();
                    const value = binding.field.readValue();
                    if (value !== null)
                        slider.value = String(value);
                }
                finally {
                    synchronizing = false;
                }
            },
            release: () => {
                unsubscribe();
                for (const target of gestureTargets)
                    target.removeEventListener('pointerdown', retainNumericGesture);
                for (const [element, attribute] of annotations)
                    element?.removeAttribute(attribute);
                field.container.removeAttribute('data-sc-number-pair');
                slider?.removeEventListener('change', change);
                slider?.remove();
            },
        };
        if (slider) {
            slider.setAttribute('aria-label', `${field.label} slider`);
            slider.dataset.scNumberSlider = '';
            field.container.setAttribute('data-sc-number-pair', '');
            slider.addEventListener('change', change);
            binding.synchronize();
            field.container.append(slider);
        }
        try {
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
/** Keep primary gesture starts away from Comfy's node drag owner; Fluent retains window move/up. */
function retainNumericGesture(event) {
    if (event instanceof MouseEvent && event.button === 0)
        event.stopPropagation();
}
