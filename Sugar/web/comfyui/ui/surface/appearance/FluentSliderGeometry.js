/** Normalize screen distances while retaining Fluent's value, stepping, and drag owners. */
export function adaptFluentSliderGeometry(slider) {
    const calculate = slider.calculateNewValue.bind(slider);
    slider.calculateNewValue = (screenDistance) => {
        const track = slider.track;
        const screenWidth = track?.getBoundingClientRect().width ?? 0;
        const layoutWidth = track?.clientWidth ?? 0;
        return calculate(screenWidth > 0 && layoutWidth > 0
            ? (screenDistance * layoutWidth) / screenWidth
            : screenDistance);
    };
}
