# Passkey Setup — In-App Enrollment Options

Passkey (WebAuthn/FIDO2) authentication is configured via Authentik blueprint (`authentik-blueprints/ifxinter-passkeys.yaml`). Users can currently register passkeys through Authentik's user settings page directly. This document outlines options for surfacing passkey enrollment from within the IFXInter UI.

## Option A: Link to Authentik User Settings (Recommended Starting Point)

Add a link in the IFXInter nav bar (e.g. next to the user's name) that opens Authentik's user settings in a new tab.

- **URL**: `{OIDC_ISSUER_BASE}/if/user/#/settings` (e.g. `http://localhost:9000/if/user/#/settings`)
- **Pros**: Zero Rust code — just an `<a>` tag. Works immediately since the user's Authentik session is already active from the OIDC login.
- **Cons**: User leaves the app. No redirect back to IFXInter after setup.

## Option B: Direct Link to the WebAuthn Setup Flow

Link directly to Authentik's user-settings flow endpoint, which drops the user into passkey enrollment more directly.

- **URL**: `{OIDC_ISSUER_BASE}/if/flow/default-user-settings-flow/`
- **Pros**: More focused — skips the full settings page and goes straight to the setup flow.
- **Cons**: Still leaves the app. Flow URL may change between Authentik versions.

## Option C: Popup Window

Open Authentik's setup flow in a browser popup window. When done, the popup closes and the user stays in IFXInter.

- **Pros**: User never leaves the app context. Better UX.
- **Cons**: More complex to implement (JavaScript popup management, detecting completion). Popup blockers may interfere. Authentik doesn't natively signal completion back to the opener window, so detecting "done" would require polling or a workaround.

## Considerations

- All options require the user to already be authenticated with Authentik (they will be, since they logged in via OIDC).
- The Authentik session cookie should still be valid, so users won't be prompted to log in again.
- Authentik does not natively redirect back to a third-party app after settings changes, which limits seamless round-trip flows.
- Option A is the pragmatic starting point. Options B and C can be explored as the UX matures.
