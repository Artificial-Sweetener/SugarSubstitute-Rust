/** Keep owned API failures accurate while bounding untrusted response text. */
export async function modelApiError(response, fallback) {
    try {
        const payload = await response.json();
        if (typeof payload === 'object' &&
            payload !== null &&
            'error' in payload &&
            typeof payload.error === 'string' &&
            payload.error.length > 0 &&
            payload.error.length <= 500)
            return new Error(payload.error);
    }
    catch (failure) {
        console.warn('SugarCubes model API returned a non-JSON error response.', failure);
    }
    return new Error(fallback);
}
