/** No lists, for an editor whose host keeps none. */
export const noTagLists = Object.freeze({
    current: () => null,
    subscribe: () => () => undefined,
});
