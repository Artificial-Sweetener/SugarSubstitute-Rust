/** Keep reusable menu input presentation independent of feature values and calculations. */
export function createContextMenuField(document, entry, changed) {
    const root = document.createElement('label');
    root.dataset.scMenuField = '';
    root.style.cssText = 'display:flex;align-items:center;gap:12px;padding:6px 12px;';
    const label = document.createElement('span');
    const input = document.createElement('input');
    input.type = 'number';
    input.min = String(entry.input.minimum);
    input.max = String(entry.input.maximum);
    input.step = entry.input.step;
    input.style.cssText =
        'width:96px;margin-left:auto;color:inherit;background:transparent;border:1px solid var(--sc-border-subtle);font:inherit;';
    root.append(label, input);
    input.addEventListener('input', () => {
        entry.input.change(input.value);
        changed();
    });
    /** Update values without replacing the focused control or normalizing incomplete decimal input. */
    const refresh = () => {
        label.textContent = entry.label;
        input.setAttribute('aria-label', entry.label);
        input.disabled = entry.disabled ?? false;
        if (input.value !== entry.input.value)
            input.value = entry.input.value;
    };
    refresh();
    return { root, input, refresh };
}
