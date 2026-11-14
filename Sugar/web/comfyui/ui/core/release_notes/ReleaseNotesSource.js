const UNAVAILABLE = Object.freeze({ kind: 'failed', reason: 'unavailable' });
/** The outcome every source reports when it cannot read notes now. */
export function unavailableNotes() {
    return UNAVAILABLE;
}
const RELEASE = /^(\d{1,9})\.(\d{1,9})\.(\d{1,9})(?:-([a-z0-9.-]+))?$/;
/**
 * Order two release tokens: numeric `major.minor.patch`, a release after its own prereleases.
 *
 * @returns A negative number, zero or a positive number, or null when either is not a version.
 */
export function compareReleaseVersions(left, right) {
    const a = RELEASE.exec(left);
    const b = RELEASE.exec(right);
    if (!a || !b)
        return null;
    for (const index of [1, 2, 3]) {
        const difference = Number(a[index]) - Number(b[index]);
        if (difference !== 0)
            return difference;
    }
    const [preA, preB] = [a[4], b[4]];
    if (preA === preB)
        return 0;
    if (preA === undefined)
        return 1;
    if (preB === undefined)
        return -1;
    return preA < preB ? -1 : 1;
}
