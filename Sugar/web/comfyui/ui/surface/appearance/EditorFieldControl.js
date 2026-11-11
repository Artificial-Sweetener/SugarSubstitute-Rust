/** Transfer native input and frame spacing into the editor so its viewport reaches the field border. */
export function adoptEditorFieldPadding(field, nativeInput) {
    const window = field.ownerDocument.defaultView;
    const frame = nativeInput === field ? field : nativeInput.parentElement;
    if (!window || !frame)
        throw new Error('The editor field requires a connected native frame.');
    // Re-read the provider's stylesheet after size/validation changes, excluding our previous transfer.
    frame.style.removeProperty('padding');
    const input = window.getComputedStyle(nativeInput);
    const outer = frame === nativeInput ? null : window.getComputedStyle(frame);
    /** Retain provider-defined spacing as content padding inside the sole scrolling surface. */
    const padding = (side) => `calc(${input[side] || '0px'} + ${outer?.[side] || '0px'})`;
    field.style.setProperty('--sc-editor-content-padding', `${padding('paddingTop')} ${padding('paddingRight')} ${padding('paddingBottom')} ${padding('paddingLeft')}`);
    field.style.setProperty('--sc-editor-content-padding-left', padding('paddingLeft'));
    frame.style.padding = '0';
}
