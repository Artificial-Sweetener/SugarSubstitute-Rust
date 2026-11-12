/** Repeat a readable title cycle independently of browser scheduling and measurement. */
export function modelTitleMarquee(elapsedMs, overflowWidth) {
    const overflow = Math.max(0, overflowWidth);
    if (overflow === 0)
        return { phase: 'start', offset: 0, leftFade: false, rightFade: false };
    const duration = Math.max(1, Math.round((overflow / 44) * 1000));
    const position = Math.max(0, elapsedMs) % (900 + duration + 700);
    if (position < 900)
        return { phase: 'start', offset: 0, leftFade: false, rightFade: true };
    if (position - 900 < duration) {
        const offset = overflow * ((position - 900) / duration);
        return { phase: 'scroll', offset, leftFade: offset > 0.5, rightFade: offset < overflow - 0.5 };
    }
    return { phase: 'end', offset: overflow, leftFade: true, rightFade: false };
}
