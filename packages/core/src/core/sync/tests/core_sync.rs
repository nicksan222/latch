// Sync against a local bare remote with two clones.

#[path = "../../../../tests/core_support/mod.rs"]
mod core_support;

use core_support::*;
use latch::core::{UnlockedVault, Vault};
use latch::protocol::*;

const BOTH: SyncParams = SyncParams {
    mode: SyncMode::Both,
};

struct World {
    _t: tempfile::TempDir,
    _remote: tempfile::TempDir,
    a: UnlockedVault,
    b: UnlockedVault,
    profile: String,
    shared: String,
}

fn secret(v: &str) -> EntryPayload {
    EntryPayload::Secret(SecretData {
        value: Some(v.into()),
    })
}

fn value(v: &UnlockedVault, id: &str) -> String {
    v.reveal(&RevealParams {
        id: id.into(),
        field: "value".into(),
    })
    .unwrap()
    .value
}

fn healthy(v: &mut UnlockedVault) {
    assert!(
        v.integrity_problems().is_empty(),
        "{:?}",
        v.integrity_problems()
    );
    let r = v
        .doctor(&DoctorParams {
            fix: false,
            rebuild_manifest: false,
            staged: false,
        })
        .unwrap();
    assert!(r.ok, "{:?}", r.issues);
}

/// A creates the vault with one profile + shared entry, pushes; B clones.
fn world() -> World {
    let remote = bare_remote();
    let t = tempfile::tempdir().unwrap();
    let mut params = init_params(&t.path().join("a"));
    params.remote_url = Some(remote.path().display().to_string());
    let (mut a, _) = Vault::init(&params).unwrap();
    let p = a.create_profile(&profile_input("acme")).unwrap();
    let shared = a
        .create_entry(&entry_input(&p.id, "shared", secret("v0")))
        .unwrap()
        .id;
    let r = a.sync(&BOTH).unwrap();
    assert_eq!(r.pushed, 3);
    assert!(r.conflicts.is_empty());
    assert_eq!((r.status.ahead, r.status.behind), (0, 0));
    assert!(r.status.last_sync_at.is_some());

    let b_path = t.path().join("b");
    let locked = Vault::clone_from(&remote.path().display().to_string(), &b_path).unwrap();
    assert!(locked.status().has_remote);
    let b = locked.unlock_passphrase(PASS).unwrap();
    assert!(b.integrity_problems().is_empty());
    assert_eq!(value(&b, &shared), "v0");
    World {
        _t: t,
        _remote: remote,
        a,
        b,
        profile: p.id,
        shared,
    }
}

fn edit(v: &mut UnlockedVault, id: &str, profile: &str, name: &str, val: &str) {
    v.update_entry(&EntryUpdateParams {
        id: id.into(),
        input: entry_input(profile, name, secret(val)),
    })
    .unwrap();
}

#[test]
fn divergent_edits_on_different_entries_merge_automatically() {
    let mut w = world();
    let xa =
        w.a.create_entry(&entry_input(&w.profile, "from-a", secret("a1")))
            .unwrap()
            .id;
    let xa2 =
        w.a.create_entry(&entry_input(&w.profile, "from-a-2", secret("a2")))
            .unwrap()
            .id;
    let yb =
        w.b.create_entry(&entry_input(&w.profile, "from-b", secret("b1")))
            .unwrap()
            .id;
    // B also renames the profile; A edits the shared entry.
    let mut pi = profile_input("acme-b");
    pi.pinned = true;
    let pid = w.profile.clone();
    w.b.update_profile(&ProfileUpdateParams {
        id: pid.clone(),
        input: pi,
    })
    .unwrap();
    edit(&mut w.a, &w.shared.clone(), &pid, "shared", "v1-a");

    assert_eq!(w.a.sync(&BOTH).unwrap().pushed, 3);
    let r = w.b.sync(&BOTH).unwrap();
    assert!(r.conflicts.is_empty(), "{:?}", r.conflicts);
    assert_eq!(r.pulled, 3);
    assert_eq!(r.pushed, 2);
    assert_eq!((r.status.ahead, r.status.behind), (0, 0));
    healthy(&mut w.b);
    for (id, val) in [(&xa, "a1"), (&xa2, "a2"), (&yb, "b1"), (&w.shared, "v1-a")] {
        assert_eq!(value(&w.b, id), val);
    }
    assert_eq!(w.b.get_profile(&pid).unwrap().name, "acme-b");

    let r = w.a.sync(&BOTH).unwrap();
    assert_eq!((r.pulled, r.pushed), (2, 0));
    healthy(&mut w.a);
    assert_eq!(value(&w.a, &yb), "b1");
    assert_eq!(
        w.a.list_entries(&ListEntriesParams {
            profile_id: None,
            query: None,
            entry_type: None
        })
        .unwrap()
        .len(),
        4
    );
    assert_eq!(w.a.list_profiles().unwrap()[0].name, "acme-b");
    // Writes keep working after the merge.
    w.a.create_entry(&entry_input(&pid, "after", secret("z")))
        .unwrap();
}

fn conflict_setup(w: &mut World) {
    let (id, pid) = (w.shared.clone(), w.profile.clone());
    edit(&mut w.a, &id, &pid, "shared", "theirs-value");
    edit(&mut w.b, &id, &pid, "shared-b", "mine-value");
    w.a.sync(&BOTH).unwrap();
    let r = w.b.sync(&BOTH).unwrap();
    assert_eq!(r.conflicts.len(), 1);
    assert_eq!(r.conflicts[0].entry_id, id);
    assert_eq!(r.conflicts[0].profile_id, pid);
    assert_eq!(r.conflicts[0].name, "shared-b");
    assert_eq!(r.pushed, 0);
    // Conflicts persist in status; writes are blocked meanwhile; sync is a no-op.
    assert_eq!(w.b.sync_status().unwrap().conflicts.len(), 1);
    assert_eq!(w.b.sync(&BOTH).unwrap().conflicts.len(), 1);
    assert_eq!(
        w.b.create_profile(&profile_input("blocked"))
            .unwrap_err()
            .code(),
        ErrorCode::Conflict
    );
    let d = w.b.conflict_detail(&id).unwrap();
    let mine = d.mine.unwrap();
    let theirs = d.theirs.unwrap();
    assert_eq!(mine.name, "shared-b");
    assert_eq!(theirs.name, "shared");
    assert_eq!(mine.payload, secret("mine-value"));
    assert_eq!(theirs.payload, secret("theirs-value"));
    assert_eq!(
        w.b.conflict_detail(&latch::core::ids::new_id())
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
}

fn finish(w: &mut World, expect_name: &str, expect_value: &str) {
    let st = w.b.sync_status().unwrap();
    assert!(st.conflicts.is_empty());
    assert_eq!((st.ahead, st.behind), (0, 0), "resolve pushes");
    healthy(&mut w.b);
    assert_eq!(value(&w.b, &w.shared), expect_value);
    w.a.sync(&BOTH).unwrap();
    healthy(&mut w.a);
    assert_eq!(value(&w.a, &w.shared), expect_value);
    assert_eq!(
        w.a.get_entry(&GetEntryParams {
            id: w.shared.clone(),
            reveal: false
        })
        .unwrap()
        .name,
        expect_name
    );
}

#[test]
fn same_entry_conflict_keep_mine() {
    let mut w = world();
    conflict_setup(&mut w);
    let st =
        w.b.resolve(&ResolveParams {
            entry_id: w.shared.clone(),
            resolution: Resolution::Mine,
            merged: None,
        })
        .unwrap();
    assert!(st.conflicts.is_empty());
    finish(&mut w, "shared-b", "mine-value");
}

#[test]
fn same_entry_conflict_keep_theirs() {
    let mut w = world();
    conflict_setup(&mut w);
    w.b.resolve(&ResolveParams {
        entry_id: w.shared.clone(),
        resolution: Resolution::Theirs,
        merged: None,
    })
    .unwrap();
    finish(&mut w, "shared", "theirs-value");
}

#[test]
fn same_entry_conflict_merged() {
    let mut w = world();
    conflict_setup(&mut w);
    let missing = w.b.resolve(&ResolveParams {
        entry_id: w.shared.clone(),
        resolution: Resolution::Merged,
        merged: None,
    });
    assert_eq!(missing.unwrap_err().code(), ErrorCode::InvalidParams);
    // Name from theirs, secret kept from mine (None = keep mine's value).
    let merged = entry_input(
        &w.profile,
        "merged-name",
        EntryPayload::Secret(SecretData { value: None }),
    );
    w.b.resolve(&ResolveParams {
        entry_id: w.shared.clone(),
        resolution: Resolution::Merged,
        merged: Some(merged),
    })
    .unwrap();
    finish(&mut w, "merged-name", "mine-value");
}

#[test]
fn modify_delete_conflict_and_multiple_stops() {
    let mut w = world();
    let pid = w.profile.clone();
    let other =
        w.a.create_entry(&entry_input(&pid, "other", secret("o0")))
            .unwrap()
            .id;
    w.a.sync(&BOTH).unwrap();
    w.b.sync(&BOTH).unwrap();
    // A deletes shared and edits other; B edits both in two commits.
    w.a.delete_entry(&w.shared.clone()).unwrap();
    edit(&mut w.a, &other, &pid, "other", "o-a");
    w.a.sync(&BOTH).unwrap();
    edit(&mut w.b, &w.shared.clone(), &pid, "shared", "s-b");
    edit(&mut w.b, &other, &pid, "other", "o-b");
    let r = w.b.sync(&BOTH).unwrap();
    assert_eq!(r.conflicts.len(), 1);
    assert_eq!(r.conflicts[0].entry_id, w.shared);
    let d = w.b.conflict_detail(&w.shared).unwrap();
    assert!(d.theirs.is_none(), "deleted upstream");
    assert!(d.mine.is_some());
    // Keep theirs = keep it deleted; the rebase then stops on `other`.
    let st =
        w.b.resolve(&ResolveParams {
            entry_id: w.shared.clone(),
            resolution: Resolution::Theirs,
            merged: None,
        })
        .unwrap();
    assert_eq!(st.conflicts.len(), 1);
    assert_eq!(st.conflicts[0].entry_id, other);
    w.b.resolve(&ResolveParams {
        entry_id: other.clone(),
        resolution: Resolution::Mine,
        merged: None,
    })
    .unwrap();
    healthy(&mut w.b);
    assert_eq!(
        w.b.get_entry(&GetEntryParams {
            id: w.shared.clone(),
            reveal: false
        })
        .unwrap_err()
        .code(),
        ErrorCode::NotFound
    );
    assert_eq!(value(&w.b, &other), "o-b");
    w.a.sync(&BOTH).unwrap();
    healthy(&mut w.a);
    assert_eq!(value(&w.a, &other), "o-b");
}

#[test]
fn abort_sync_restores_local_state() {
    let mut w = world();
    conflict_setup(&mut w);
    let st = w.b.abort_sync().unwrap();
    assert!(st.conflicts.is_empty());
    assert_eq!(st.ahead, 1);
    assert_eq!(st.behind, 1);
    healthy(&mut w.b);
    assert_eq!(value(&w.b, &w.shared), "mine-value");
}

#[test]
fn fast_forward_pull_and_modes() {
    let mut w = world();
    let pid = w.profile.clone();
    let e =
        w.a.create_entry(&entry_input(&pid, "new", secret("n")))
            .unwrap()
            .id;
    w.a.sync(&BOTH).unwrap();
    // B has a local commit and is behind: push-only refuses.
    w.b.create_entry(&entry_input(&pid, "local", secret("l")))
        .unwrap();
    let err =
        w.b.sync(&SyncParams {
            mode: SyncMode::PushOnly,
        })
        .unwrap_err();
    assert_eq!(err.code(), ErrorCode::Conflict);
    // Pull-only rebases but does not push.
    let r =
        w.b.sync(&SyncParams {
            mode: SyncMode::PullOnly,
        })
        .unwrap();
    assert_eq!((r.pulled, r.pushed), (1, 0));
    assert_eq!(r.status.ahead, 1);
    assert_eq!(value(&w.b, &e), "n");
    let r =
        w.b.sync(&SyncParams {
            mode: SyncMode::PushOnly,
        })
        .unwrap();
    assert_eq!(r.pushed, 1);
    // A: pure fast-forward.
    let r = w.a.sync(&BOTH).unwrap();
    assert_eq!((r.pulled, r.pushed), (1, 0));
    healthy(&mut w.a);
    assert!(!r.status.log.is_empty());
    assert!(r.status.remote_url.is_some());
}

#[test]
fn upstream_rotation_is_picked_up() {
    let mut w = world();
    let pid = w.profile.clone();
    w.a.rotate().unwrap();
    w.a.sync(&BOTH).unwrap();
    // B wrote with the old key meanwhile.
    let local =
        w.b.create_entry(&entry_input(&pid, "pre-rotate", secret("old-key")))
            .unwrap()
            .id;
    let rel = latch::core::format::entry_file(&pid, &local);
    let before = std::fs::read(w.b.path().join(&rel)).unwrap();
    let r = w.b.sync(&BOTH).unwrap();
    let after = std::fs::read(w.b.path().join(&rel)).unwrap();
    assert_ne!(before, after, "re-encrypted to the rotated key");
    assert!(r.conflicts.is_empty());
    healthy(&mut w.b);
    assert_eq!(value(&w.b, &local), "old-key");
    assert_eq!(value(&w.b, &w.shared), "v0");
    w.a.sync(&BOTH).unwrap();
    healthy(&mut w.a);
    assert_eq!(value(&w.a, &local), "old-key");
    let ct = std::fs::read(w.a.path().join(&rel)).unwrap();
    assert_eq!(ct, after);
    assert!(!w.a.decrypt_blob(&ct).unwrap().is_empty());
}

#[test]
fn concurrent_recipient_changes_abort() {
    let mut w = world();
    let k1 = age::x25519::Identity::generate().to_public().to_string();
    let k2 = age::x25519::Identity::generate().to_public().to_string();
    w.a.add_recipient(&AddRecipientParams {
        label: "one".into(),
        public_key: k1,
    })
    .unwrap();
    w.a.sync(&BOTH).unwrap();
    w.b.add_recipient(&AddRecipientParams {
        label: "two".into(),
        public_key: k2,
    })
    .unwrap();
    let e = w.b.sync(&BOTH).unwrap_err();
    assert_eq!(e.code(), ErrorCode::Conflict);
    let st = w.b.sync_status().unwrap();
    assert!(st.conflicts.is_empty());
    healthy(&mut w.b);
}

#[test]
fn no_remote_and_set_remote() {
    let t = tempfile::tempdir().unwrap();
    let mut v = new_vault(&t.path().join("v"));
    let r = v.sync(&BOTH).unwrap();
    assert_eq!((r.pulled, r.pushed), (0, 0));
    assert!(!r.status.has_remote);
    let remote = bare_remote();
    let st = v.set_remote(&remote.path().display().to_string()).unwrap();
    assert!(st.has_remote);
    let r = v.sync(&BOTH).unwrap();
    assert_eq!(r.pushed, 1);
    let st = v.set_remote("").unwrap();
    assert!(!st.has_remote);
    assert!(v.status().ahead == 0);
}
