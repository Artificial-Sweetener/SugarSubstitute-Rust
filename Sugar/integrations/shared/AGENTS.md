# Shared adapter runtime

- Own protocol validation, request correlation, bounded transfers, session rules, cancellation, and timeouts here when those behaviors are introduced.
- Depend on `integrations/contracts` and portable collaborators supplied explicitly by callers.
- Browser, network, storage, and editor APIs belong behind injected typed ports implemented by the host integration.
- Exercise each shared operation against a capable host and a deliberately limited in-memory host through one conformance suite.
