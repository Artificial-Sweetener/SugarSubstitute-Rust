/** Identify durable source associations on their owning workflow node. */
export const MASK_BINDING_KEY = 'sugarcubes_canvas_masks';
/** Identify image-field bindings while retaining the public mask workflow format. */
export const IMAGE_BINDING_KEY = 'sugarcubes_canvas_images';
/** Keep selection attached to identity while moving entries through an ordered batch. */
export function moveInputEntry(binding, id, delta) {
    const entries = [...binding.entries];
    const index = entries.findIndex((entry) => entry.id === id), next = index + delta;
    if (index < 0 || next < 0 || next >= entries.length)
        return binding;
    const value = entries[index];
    const other = entries[next];
    if (!value || !other)
        return binding;
    entries[index] = other;
    entries[next] = value;
    return { ...binding, entries };
}
