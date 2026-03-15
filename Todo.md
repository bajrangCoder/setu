# TODO
- [ ] Apply API Key auth in query params when "Add to: Query Param" is selected (`AuthEditor` supports it, request pipeline does not).
- [x] Restore full request state when loading from history/collections (headers are not restored; method/body consistency has edge cases).
- [ ] Add a visible "Save request to collection" flow in UI (collections can be created/loaded/deleted, but saving current request is not exposed).
- [x] Support canceling in-flight requests
- [x] Fix "Save response" for binary/image responses to write raw bytes, not text.
- [x] Ensure duplicated requests preserve method/body/headers exactly.

- [ ] Environment variables and secret variables (`{{base_url}}`, tokens, etc.) with workspace/project scopes.
- [ ] Import/Export: cURL import, request/collection JSON export, and shareable files.
- [ ] Implement protocol modes marked "SOON": WebSocket, GraphQL, and SSE.
- [ ] Stronger auth support: OAuth 2.0 flows, Digest auth, and API key conveniences.
- [ ] Cookie jar/session persistence with per-domain controls.
- [ ] Better response rendering: HTML preview, XML pretty print, better binary metadata/download UX.
