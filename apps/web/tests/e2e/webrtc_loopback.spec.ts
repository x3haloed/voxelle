import { test, expect } from '@playwright/test'

test('WebRTC transport loopback connects and delivers messages', async ({ page, baseURL }) => {
  await page.goto(baseURL!)

  const result = await page.evaluate(async () => {
    const m = await import('/src/voxelle/webrtc.ts')

    const a = m.createWebRtcTransport({ iceServers: [] })
    const b = m.createWebRtcTransport({ iceServers: [] })

    let received: any = null
    b.onMessage((msg: any) => {
      received = msg
    })

    const offer = await a.startOffer()
    const answer = await b.acceptOfferAndMakeAnswer(offer)
    await a.acceptAnswer(answer)

    await new Promise<void>((resolve, reject) => {
      const start = Date.now()
      const tick = () => {
        const as = a.getState().state
        const bs = b.getState().state
        if (as === 'connected' && bs === 'connected') return resolve()
        if (Date.now() - start > 20_000) return reject(new Error(`timeout waiting connect: a=${as} b=${bs}`))
        setTimeout(tick, 50)
      }
      tick()
    })

    a.send({ t: 'ping', v: 1 })

    await new Promise<void>((resolve, reject) => {
      const start = Date.now()
      const tick = () => {
        if (received) return resolve()
        if (Date.now() - start > 5_000) return reject(new Error('timeout waiting message'))
        setTimeout(tick, 25)
      }
      tick()
    })

    const out = {
      a: a.getState().state,
      b: b.getState().state,
      received,
    }
    a.close()
    b.close()
    return out
  })

  expect(result.a).toBe('connected')
  expect(result.b).toBe('connected')
  expect(result.received).toEqual({ t: 'ping', v: 1 })
})

