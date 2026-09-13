# Web-console demo checks

From the repository root:

```sh
npm ci
npm --prefix web ci
npx playwright install chromium
npm run test:web
```

The suite starts its own loopback-only Vite server on port 5178 and checks the local demo in Chromium. It covers disabled unimplemented actions, fit/native viewing versus simulated source resolution, visible resize failure, close/reopen focus, mobile overflow, persistent light/dark themes and fullscreen exit. It does not contact Spark, Keycloak or a broker and cannot establish real desktop control, security or 4K quality.

For an already-installed compatible Chromium, set `DESKTOP_TEST_CHROMIUM` to its absolute executable path. Browser download is then unnecessary. Failure output belongs under gitignored `artifacts/web-tests/`.
