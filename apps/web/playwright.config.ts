import { defineConfig } from '@playwright/test'

// Avoid clashing with a dev server already running on 5173.
const port = Number(process.env.VOXELLE_TEST_PORT ?? '5174')

export default defineConfig({
  testDir: './tests/e2e',
  timeout: 180_000,
  expect: { timeout: 20_000 },
  use: {
    baseURL: `http://localhost:${port}`,
    headless: true,
    viewport: { width: 1200, height: 800 },
    launchOptions: {
      args: [
        // Helps local WebRTC in automation where mDNS candidates can be flaky.
        '--disable-features=WebRtcHideLocalIpsWithMdns',
      ],
    },
  },
  // Spin up the web UI for tests. Signaling relay is started by the spec (per-test lifecycle).
  webServer: {
    command: `npm run dev -- --port ${port} --strictPort`,
    url: `http://localhost:${port}`,
    reuseExistingServer: !process.env.CI,
    stdout: 'pipe',
    stderr: 'pipe',
  },
})
