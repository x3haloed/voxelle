# Voxelle Field Test

This is the smallest path for a browser-based friend test through localhost.run.

## Start

```bash
npm run field:test
```

By default, the launcher uses localhost.run's anonymous SSH target, `nokey@localhost.run`.

If you have a localhost.run account/key and want to use it instead, run:

```bash
VOXELLE_LOCALHOST_RUN_TARGET=localhost.run npm run field:test
```

The launcher starts:

- the Vite web app on `127.0.0.1:5173`
- `voxelle-signal` on `127.0.0.1:9002`
- one localhost.run HTTPS tunnel for the app
- one localhost.run HTTPS/WebSocket tunnel for the signaling relay

It prints an App URL and a Relay URL. Keep the terminal open while testing.

## Test Checklist

1. Open the App URL yourself.
2. Create a Space.
3. In the Space Invite panel, paste the Relay URL.
4. Click `Create Invite (copy link)`.
5. Open the generated host link in your browser so your side starts hosting the relay session.
6. Send the invite link to one tester.
7. Ask the tester to open it, join `#general`, and wait for connection status `connected`.
8. Both sides send a short unique message.
9. Refresh both browsers and confirm messages remain.

## Current Limitation

The current web client manages one WebRTC peer connection per room tab. This launcher is meant to prove public serving and relay rendezvous. True five-person group chat still needs a multi-peer transport pass.

For a five-person session today, use this as a pairwise smoke test with one tester at a time, or treat the failure to form a five-way room as the next product priority.
