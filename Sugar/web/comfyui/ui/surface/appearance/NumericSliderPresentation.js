/** Use GIMP spin scales for bounded properties and retain the other providers' established float policy. */
export function usesNumericSlider(field, style) {
    const bounded = [field.minimum, field.maximum, field.step].every(Number.isFinite) &&
        field.maximum > field.minimum &&
        field.step > 0 &&
        (field.maximum - field.minimum) / field.step <= 2_147_483_647;
    if (!bounded)
        return false;
    // GIMP's size-entry dimensions use ordinary spinbuttons, while tool properties use spin scales.
    if (style === 'gimp')
        return !['width', 'height'].includes(field.name);
    return usesSliderSpinner(field);
}
/** Preserve conventional slider-and-spinner fields when selecting GIMP's numeric control variant. */
export function usesGimpSpinScale(field) {
    return usesNumericSlider(field, 'gimp') && !usesSliderSpinner(field);
}
/** Share the established slider-and-spinner semantics across visual-style providers. */
function usesSliderSpinner(field) {
    const label = field.label.trim().toLowerCase().replace(/[_-]/g, ' ').replace(/\s+/g, ' ');
    return (field.presentation === 'spinner-slider' ||
        (field.precision !== 0 &&
            (['denoise', 'variation_strength'].includes(field.name) || label === 'scale factor')));
}
