---
name: latch-managed-browser
description: Implement or investigate Latch's isolated, app-managed Firefox runtime, login seeding, and encrypted session persistence.
---

# Latch managed browser

Latch owns the Firefox runtime and profile lifecycle. Do not discover or launch a system browser, add an extension or native messaging, or create page-context access to vault secrets.

Each workspace gets a separate Firefox profile. Eligible website logins are seeded one way into Firefox's native password manager; browser edits do not update Latch. Session archives are encrypted, and decrypted session data stays outside the vault. Locking closes managed browsers.

Profile separation is not an OS security boundary against malware running as the user. The runtime is pinned and checksum-verified; update its pin through the maintained runtime workflow, not by making browser updates automatic. Keep platform claims tied to observed verification results.
