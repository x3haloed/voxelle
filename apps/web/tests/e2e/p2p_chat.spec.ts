import { test, expect } from '@playwright/test'

// Full UI-level two-context WebRTC can be flaky under automation in some environments.
// Enable explicitly when you want to validate the whole flow end-to-end:
//   VOXELLE_E2E_UI=1 npm run test:e2e -w @voxelle/web
test.skip(!process.env.VOXELLE_E2E_UI, 'VOXELLE_E2E_UI not set')

function decodeBase64UrlToUtf8(s: string): string {
  const b64 = s.replace(/-/g, '+').replace(/_/g, '/')
  const pad = '='.repeat((4 - (b64.length % 4)) % 4)
  return Buffer.from(b64 + pad, 'base64').toString('utf8')
}

test.describe('Voxelle MVP: 2 peers chat over WebRTC', () => {
  test('invite → rendezvous relay → connect → message sync', async ({ browser, baseURL }) => {
    // Owner context
    const ownerContext = await browser.newContext()
    await ownerContext.addInitScript(() => {
      localStorage.setItem('voxelle.seeded.v1', '1')
    })
    const owner = await ownerContext.newPage()
    await owner.goto(baseURL!)

    await owner.getByPlaceholder(/New space name/i).fill('E2E Space')
    await owner.getByRole('button', { name: /create space/i }).click()
    await owner.waitForURL(/\/s\//)

    await owner.getByRole('button', { name: /create invite/i }).click()

    const inviteLink = await owner.locator('textarea').first().inputValue()
    expect(inviteLink).toContain('#invite=')

    const inviteUrl = new URL(inviteLink)
    const frag = inviteUrl.hash.startsWith('#') ? inviteUrl.hash.slice(1) : inviteUrl.hash
    const params = new URLSearchParams(frag)
    const invB64u = params.get('invite')
    expect(invB64u).toBeTruthy()
    const inv = JSON.parse(decodeBase64UrlToUtf8(invB64u!))
    const spaceId: string = String(inv.space_id)
    const sid: string = String(inv.invite_id)

    // Host page (owner) goes to the room and creates an offer (manual signaling).
    const host = await ownerContext.newPage()
    await host.goto(`${baseURL}/s/${encodeURIComponent(spaceId)}/r/${encodeURIComponent('room:general')}`)
    await host.waitForURL(/\/r\//)
    await expect(host.getByPlaceholder(/Message/i)).toBeVisible({ timeout: 10_000 })
    // For local e2e, disable external STUN to avoid network dependency.
    await host.getByPlaceholder(/stun:/i).fill('')
    await host.getByRole('button', { name: /^host$/i }).click()
    const offerOut = await host.locator('textarea').first().inputValue()
    expect(offerOut.trim().length).toBeGreaterThan(20)

    // Joiner context (fresh localStorage) consumes invite and auto-joins relay.
    const joinerContext = await browser.newContext()
    await joinerContext.addInitScript(() => {
      localStorage.setItem('voxelle.seeded.v1', '1')
    })
    const joiner = await joinerContext.newPage()
    await joiner.goto(inviteLink)
    await joiner.waitForURL(/\/s\//)
    await joiner.getByRole('link', { name: /#general/i }).click()
    await joiner.waitForURL(/\/r\//)
    await expect(joiner.getByPlaceholder(/Message/i)).toBeVisible({ timeout: 10_000 })
    await joiner.getByPlaceholder(/stun:/i).fill('')

    const connectStatus = (page: any) => page.getByTestId('connection-panel').getByTestId('transport-status')

    // Joiner pastes offer, creates answer, sends back; host accepts answer.
    await joiner.getByPlaceholder(/Paste offer code/i).fill(offerOut)
    await joiner.getByRole('button', { name: /create answer/i }).click()
    const answerOut = await joiner.locator('textarea').first().inputValue()
    expect(answerOut.trim().length).toBeGreaterThan(20)

    await host.getByPlaceholder(/Paste answer code/i).fill(answerOut)
    await host.getByRole('button', { name: /accept answer/i }).click()

    await expect(connectStatus(host)).toHaveText('connected', { timeout: 60_000 })
    await expect(connectStatus(joiner)).toHaveText('connected', { timeout: 60_000 })

    const msg = `hello-${Date.now()}`
    await host.getByPlaceholder(/Message/i).fill(msg)
    await host.getByRole('button', { name: /^send$/i }).click()

    await expect(joiner.getByText(msg)).toBeVisible({ timeout: 20_000 })
  })
})
