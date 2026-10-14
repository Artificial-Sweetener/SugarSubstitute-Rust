/** Scale HSV value while retaining accent hue and saturation. */
export function orbStateAccent(accent, state) {
    const scale = state === 'pressed' ? 0.88 : state === 'hover' ? 1.08 : 1;
    const maximum = Math.max(...accent);
    const value = Math.min(255, Math.round(maximum * scale));
    const channel = (input) => (maximum ? Math.round((input * value) / maximum) : 0);
    return [channel(accent[0]), channel(accent[1]), channel(accent[2])];
}
/** Transform one source pixel with the original glass-highlight and accent-shading formulas. */
export function tintOrbPixel(source, accent) {
    const [red, green, blue, alphaChannel] = source;
    if (!alphaChannel)
        return [0, 0, 0, 0];
    const luminance = Math.round(red * 0.299 + green * 0.587 + blue * 0.114) / 255;
    const smooth = (value) => {
        const x = Math.max(0, Math.min(1, value));
        return x * x * (3 - 2 * x);
    };
    const highlight = Math.min(1, smooth((luminance - 0.8) / 0.2) ** 1.02 * (1 - (alphaChannel / 255) ** 5) * 1.18);
    const shade = 0.58 + 0.42 * smooth(luminance);
    const channel = (value) => Math.round(value * shade + (255 - value * shade) * highlight);
    return [channel(accent[0]), channel(accent[1]), channel(accent[2]), alphaChannel];
}
