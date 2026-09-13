# Web-console demo validation — 2026-09-06

Scope: local React UI fixtures only, no Spark input, Keycloak realm changes or public deployment. Andrew accepted starting UI development before all engine gates pass.

Results:

- Existing Node contract/model suite: 105 passed.
- Web Vitest state/coordinate suite: 7 passed.
- TypeScript check and Vite production build: passed.
- Chromium integration suite: 3 passed (demo control success/failure and focus; 390px mobile overflow; theme persistence and fullscreen enter/exit).
- `git diff --check`: passed at validation time.

Browser invocation used cached Chromium with `DESKTOP_TEST_CHROMIUM=/home/localuser/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome npm run test:web`. Reproduction instructions are in `tests/integration/README.md`. Compact 1280×900 preview screenshots are optional using `DESKTOP_TEST_SCREENSHOTS=1`; they are illustrations of the UI, not live Spark captures.

Theme reference: DonkeyWork Agents plus both Obsidian DonkeyWork design-system notes. Root visually inspected the dark preview. Inter/JetBrains Mono use system fallbacks if not installed; no external font service is requested.

Not validated: real login/logout, attachment authorization, real online status, frame transport/decoding, live pointer/keyboard, terminal, actual remote resolution changes or 4K performance. Draft broker-boundary review is recorded in `contracts/web-broker-proposal.md`; it is not an implemented API.
