# Latch vault

This repository is a [Latch](https://github.com/) secret vault (vault id `n5r5ucvcut4eyicgzaxvr2hxoq`).

Everything under `profiles/` and every `*.age` file is encrypted with
[age](https://age-encryption.org). Names, usernames, URLs and values are inside
the ciphertext. The repository is designed to be safe to publish: a reader learns
only the number and approximate size of entries, the KDF parameters, the
recipient public keys and commit metadata.

## Layout

- `.latch/version` - format version
- `.latch/config.toml` - public configuration (KDF parameters, recipients)
- `.latch/keychain.json` - signed history of the vault signing keys
- `.latch/vaultkey.age` - vault key, encrypted to every recipient
- `.latch/manifest.age`, `.latch/manifest.sig` - signed, encrypted index
- `profiles/<id>/profile.age`, `profiles/<id>/entries/<id>.age` - one encrypted file each

## Usage

```
latch unlock
latch ls
latch env <profile> -- <command>
```

Do not edit files in this repository by hand. Use `latch doctor` to verify it.
