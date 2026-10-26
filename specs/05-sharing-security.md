# Library exchange, identity and security

## Supported deployment modes

| Mode | Infrastructure | Intended capability |
|---|---|---|
| Offline device | None after install | Read, annotate, search, export/import files |
| Local exchange | Two foreground devices on reachable LAN | Explicitly paired transfer; no internet account |
| Personal mesh | Tailscale service OR self-hosted Headscale | Private reachability; app-level authorization still required |
| Plain WireGuard | User-managed peers/routes/keys | Same HTTPS endpoints; no Tailscale API requirement |
| Personal hub | Static web + OIDC + encrypted sync | Optional multi-device identity and note transport |
| Maintainer | Ingestion/review worker + database | Produce reviewed packages for others to import |

Tailscale uses WireGuard for its data plane, but WireGuard alone does not supply Tailscale coordination, discovery, policy and relays [S13]. Headscale supports a self-hosted coordination path and DERP configuration [S14]. Test the pinned mobile clients against the chosen Headscale release. App identity is separate from network membership.

Use the OS VPN client initially rather than embedding a second VPN implementation. Native apps connect to ordinary endpoints over the existing tunnel. Browser apps depend on the host's network path; they cannot act as arbitrary WireGuard peers. Tailnet IP possession and HTTP identity headers are not sufficient app authentication. Private endpoints still use HTTPS, least privilege and explicit pairing/auth.

## Package exchange protocol v1

An approved public package is a deterministic archive with a versioned manifest, safe filenames, SHA-256 hashes, chunk list, size ceilings, required reader/schema versions, edition provenance and machine-readable rights. Sign canonical manifest bytes with an established signature library; key ID and public trust root accompany verification policy. Signatures authenticate the publisher and detect changes; they do not establish theological or textual truth.

Transfer sequence: user chooses editions → client derives permitted share set → receiver sees title/size/rights/publisher and accepts → peers authenticate → exchange manifest → receiver requests missing chunks → verify each chunk and final artifact hash → stage install → atomically activate. A disconnection resumes from verified chunks without restarting or changing the active version. File export/import carries the same package format.

For LAN native exchange, a spike tests a foreground-only HTTPS endpoint paired through QR containing endpoint, short-lived capability and certificate/public-key fingerprint; manual code is a fallback. Use standard TLS/crypto libraries and have the protocol reviewed. A short code requires a reviewed PAKE or comparably safe channel, not a guessed password protocol. Capabilities expire and are single-purpose; do not log them.

No global DHT, anonymous public seeding, automatic library advertisement or unsolicited discovery. LAN discovery is explicit opt-in and scoped to the share screen. Unknown publisher keys show 'untrusted source' and require a trust/import decision; never auto-execute packages. Verified source trusts and user-imported packages remain distinguishable.

## Platform reality

Native iOS/Android foreground exchange is the primary LAN path. OS background limits mean a phone is not a reliable seed server. An always-on optional hub can serve approved packages. File exchange via native share sheet, removable storage where available, or other user-chosen transport is the universal fallback.

The web supports file import/export first. Browser-to-browser WebRTC can be investigated later but needs signaling and sometimes TURN; it is not a server-free universal solution. A browser cannot usually listen on arbitrary LAN sockets, pin certificates as a native app can, or discover arbitrary nearby devices. HTTPS, local-network permissions, CORS/private-network restrictions and storage limits require platform-specific tests. Never claim parity before those tests pass.

Remote direct peers can fail behind NAT/firewalls; relay bandwidth and hosting remain a real cost. The self-host profile can run its own DERP and exclude public relays after connectivity testing [S14]. Previously paired local/file exchange remains available if coordination is down. New remote pairing and changed routes may not.

## Identity and encrypted notes

Use self-hosted OIDC discovery and Authorization Code + PKCE S256, exact redirect allowlists and external system-browser login. Web redirects avoid assuming popup compatibility with storage isolation headers. Never embed a confidential client secret in mobile/web bundles. Validate issuer/audience/expiry; store mobile credentials in OS secure storage and design browser session exposure explicitly.

Use `(issuer, subject)` as remote principal identity, linked to an independently generated local vault ID through a user-approved enrollment. Changing issuer is not an automatic identity match by email. Provide a tested export/link migration and account deletion process. Identity-server administration cannot silently decrypt client notes.

Private note sync is end-to-end encrypted using a reviewed maintained library/protocol. Encryption and signing keys are distinct; do not design new cryptography. A vault key is wrapped to approved device keys and a user-held recovery secret. Device enrollment requires confirmation from an existing device or recovery material. Record device provenance and reject replayed/unauthorized operations.

Lost device: revoke future sync authorization, rotate relevant wrapping/session keys, rewrap vault key as required, and document that previously obtained plaintext/keys cannot be recalled. Lost all keys: no promise of server recovery. Provide a tested offline recovery export and understandable recovery setup. Server account deletion removes server ciphertext per retention policy, not other people's or offline devices' copies.

## Threat boundaries and tests

- Threats addressed: malicious packages, accidental disclosure, network eavesdropping, unauthorized sync access, poisoned catalog data and compromised ingestion workers.
- Limitations: device seizure/unlocked endpoints, OS compromise, screenshots, malicious recipients, traffic analysis, restrictive network blocking and physical coercion. VPN/E2EE is not a claim of anonymity or safety in a closed country.
- Metadata: server may see account, connection timing, IP and ciphertext sizes. Catalog requests and public hashes may reveal interests. No default analytics, push dependency, remote covers or reading-history logs.
- Sensitive mode: explicit local-only operation, network activity indicator, no background discovery, clear OS backup policy and optional encrypted local vault. Explain web-origin/server-delivered-code trust: a malicious host can serve code that steals keys. Native signed releases and static integrity checks reduce different risks but cannot defeat endpoint compromise.
- Local database encryption is evaluated separately from transport/E2EE. OS lock/backup protection is baseline; SQLCipher/platform equivalent requires a tested key/recovery design. No unsupported secure-erasure promise on flash storage.

Release tests include wrong peer/fingerprint, reused capability, invalid signature, rollback/replay, package bomb, duplicate archive entries, revoked device, offline restart, interrupted transfer, hostile LAN, no-public-relay topology and injected prompt text. A deliberate old-version install is a visible override with retained provenance, not an unnoticed downgrade.

## Revision 2 privacy boundaries

Personal notes, chats, captured views and ink are excluded from public library packages. Snapshot/export previews reveal any selected private material. Vault bridges are scoped to a user-approved path and cannot execute note content. The research companion receives only selected context; it has a different trust boundary from opaque E2EE sync. Provider credentials are isolated per user and never embedded in vaults, share bundles or Nix store paths. See specs/13-obsidian-and-ink.md and specs/14-research-assistant.md for detailed controls and release tests.
