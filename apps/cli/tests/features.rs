//! CLI integration tests cover command routing, presentation and consent.
//! Domain behavior contracts belong to `latch-core`.

#[path = "support/mod.rs"]
mod cli_support;
#[path = "features/environment.rs"]
mod environment;

mod secrets {
    use super::cli_support::Env;

    #[test]
    fn set_routes_profile_entry_arguments_and_reports_human_output() {
        let env = Env::standalone();
        env.init();
        env.ok(&["profile", "add", "acme"]);

        const SECRET: &str = "cli-interface-private-token";
        let (code, out, err) = env.run(&[
            "set",
            "acme/service",
            "--type",
            "login",
            "username=alice",
            &format!("password={SECRET}"),
        ]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("created acme/service"));
        assert!(!out.contains(SECRET) && !err.contains(SECRET));

        let (code, out, err) = env.run(&["ls", "acme", "--json"]);
        assert_eq!(code, 0, "{err}");
        let entries: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(entries.is_array(), "--json must produce a JSON array");
    }

    #[test]
    fn non_tty_reveal_requires_yes_and_keeps_secret_out_of_error_output() {
        let env = Env::standalone();
        env.init();
        env.ok(&["profile", "add", "acme"]);

        const SECRET: &str = "cli-interface-private-token";
        env.ok(&[
            "set",
            "acme/service",
            "--type",
            "login",
            &format!("password={SECRET}"),
        ]);

        let (code, out, err) = env.run(&["get", "acme/service", "--json", "--reveal"]);
        assert_ne!(code, 0);
        assert!(err.contains("--yes"));
        assert!(!out.contains(SECRET) && !err.contains(SECRET));

        let (code, out, err) = env.run(&["get", "acme/service", "--json", "--reveal", "--yes"]);
        assert_eq!(code, 0, "{err}");
        let _: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(
            out.contains(SECRET),
            "explicitly confirmed reveal prints the value"
        );
    }
}

mod profiles {
    use super::cli_support::Env;

    #[test]
    fn name_resolution_json_output_and_delete_confirmation() {
        let env = Env::standalone();
        env.init();

        let (code, out, err) = env.run(&["profile", "add", "Acme"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("created profile Acme"));
        assert!(out.is_empty());

        let (code, out, err) = env.run(&["profile", "show", "Acme", "--json"]);
        assert_eq!(code, 0, "{err}");
        let profile: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(profile["name"], "Acme");

        let (code, out, err) = env.run(&["profile", "rm", "Acme"]);
        assert_ne!(code, 0);
        assert!(err.contains("--yes"));
        assert!(out.is_empty());

        let (code, out, err) = env.run(&["profile", "rm", "Acme", "--yes"]);
        assert_eq!(code, 0, "{err}");
        assert!(err.contains("deleted profile Acme"));
        assert!(out.is_empty());
    }
}
