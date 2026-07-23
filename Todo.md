# TODO
- [x] Apply API Key auth in query params when "Add to: Query Param" is selected (`AuthEditor` supports it, request pipeline does not).
- [x] Restore full request state when loading from history/collections (headers are not restored; method/body consistency has edge cases).
- [x] Add a visible "Save request to collection" flow in UI (collections can be created/loaded/deleted, but saving current request is not exposed).
- [x] Support canceling in-flight requests
- [x] Fix "Save response" for binary/image responses to write raw bytes, not text.
- [x] Ensure duplicated requests preserve method/body/headers exactly.

- [ ] Import/Export: cURL import, request/collection JSON export, and shareable files.
- [ ] Finish Postman import coverage: scripts/tests, saved response examples, certificates, and bulk data-dump folder import.
- [ ] Implement protocol modes marked "SOON": WebSocket, GraphQL, and SSE.
- [ ] Stronger auth support: OAuth 2.0 flows, Digest auth, and API key conveniences.
- [ ] Cookie jar/session persistence with per-domain controls.
- [ ] Better response rendering: HTML preview, XML pretty print, better binary metadata/download UX.

## Environments & Workspaces

- [x] Named environments with `{{variable}}` interpolation across URLs, params, headers, auth, and bodies.
- [x] Global variables plus workspace and collection/project overrides.
- [x] Local secret values, enable/disable controls, nested variables, environment colors, and duplication.
- [x] Grouped environment manager with separate edit and active states, preset colors, and a full custom color picker.
- [x] Real persisted workspace containers and a top-bar switcher that isolate collections, history, and environments.
- [x] Import Postman collections as new workspaces, plus environment exports and collection variables in their appropriate workspace.
- [ ] Scope UI preferences and restorable request-tab sessions per workspace.
- [ ] Encrypt secret values with an OS-keychain-backed key instead of relying only on local file permissions.
- [ ] Export environment files; exported secret values must be omitted.
- [ ] Variable autocomplete, syntax highlighting, scope badges, and resolved-value previews in request editors.
- [ ] Initial/current values and temporary session overrides.
- [ ] Request-scoped, folder-scoped, predefined (`$timestamp`, `$uuid`), and OS environment variables.
- [ ] Pre-request/test scripting APIs for reading and updating variables.
- [ ] Git/team workspace sync, conflict handling, roles, and per-user secret values.
