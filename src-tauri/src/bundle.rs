//! Bundle export / import — compares IDENTITY, not name (requirements §9).

use crate::model::{Bundle, Profile};
use serde::Serialize;

/// What to do with one incoming profile, given what already exists locally.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImportPlan {
    /// Identical — skip silently, do NOT spawn a copy.
    Skip { alias: String },
    /// Brand new — keeps its name.
    Add { alias: String },
    /// Same name but DIFFERENT thing → auto-rename, no prompt.
    Rename { from: String, to: String },
}

/// Picks a free alias when the name is taken: `cht` → `cht-1` → `cht-2`.
///
/// A dash, NOT `(1)` like a downloaded file — an alias is typed at a terminal,
/// and spaces or parentheses make it unusable.
fn next_free_alias(base: &str, taken: &[String]) -> String {
    if !taken.iter().any(|t| t == base) {
        return base.to_string();
    }
    (1..)
        .map(|n| format!("{base}-{n}"))
        .find(|cand| !taken.iter().any(|t| t == cand))
        .expect("an unbounded sequence always yields a free name")
}

/// Plans an import. Runs straight through — NO prompts.
pub fn plan_import(incoming: &Bundle, existing: &[Profile]) -> Vec<ImportPlan> {
    let mut taken: Vec<String> = existing.iter().map(|p| p.alias.clone()).collect();
    let mut plans = Vec::new();

    for inc in &incoming.profiles {
        // 1. Do we already hold something IDENTICAL? (regardless of its name)
        if existing.iter().any(|e| e.is_identical_to(inc)) {
            plans.push(ImportPlan::Skip { alias: inc.alias.clone() });
            continue;
        }

        // 2. Is the name taken by something else?
        let free = next_free_alias(&inc.alias, &taken);
        if free == inc.alias {
            plans.push(ImportPlan::Add { alias: free.clone() });
        } else {
            plans.push(ImportPlan::Rename { from: inc.alias.clone(), to: free.clone() });
        }
        taken.push(free);
    }

    plans
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CliKind, DangerLevel, ModelMap, Origin};

    fn profile(alias: &str, provider: &str, key: &str) -> Profile {
        Profile {
            alias: alias.into(),
            cli: CliKind::Claude,
            provider: provider.into(),
            base_url: format!("https://{provider}.example/v1"),
            api_key: key.into(),
            env_var: "TEST_KEY".into(),
            danger: DangerLevel::Bypass,
            model_map: ModelMap::default(),
            wire_api: None,
            origin: Origin::Manual,
        }
    }

    /// The lesson from seven duplicated `# Added by Antigravity` lines:
    /// re-importing the same bundle must not produce cht-1, cht-2, cht-3.
    #[test]
    fn reimporting_same_bundle_creates_no_duplicates() {
        let existing = vec![profile("cht", "htmustc", "k1")];
        let incoming = Bundle::new(vec![profile("cht", "htmustc", "k1")]);

        let plans = plan_import(&incoming, &existing);
        assert_eq!(plans, vec![ImportPlan::Skip { alias: "cht".into() }]);
    }

    /// Same name, different provider = a real conflict, so rename.
    #[test]
    fn same_alias_different_identity_gets_suffixed() {
        let existing = vec![profile("cht", "htmustc", "k1")];
        let incoming = Bundle::new(vec![profile("cht", "otherprov", "k2")]);

        let plans = plan_import(&incoming, &existing);
        assert_eq!(
            plans,
            vec![ImportPlan::Rename { from: "cht".into(), to: "cht-1".into() }]
        );
    }

    /// Same provider but the key changed → NOT identical.
    ///
    /// This is the "rotated the key on machine A, carried it to machine B"
    /// case. Treating it as identical leaves machine B silently holding a dead
    /// key, which is exactly the kind of silent-success failure this tool
    /// exists to prevent.
    #[test]
    fn changed_key_is_not_identical() {
        let existing = vec![profile("cht", "htmustc", "old")];
        let incoming = Bundle::new(vec![profile("cht", "htmustc", "new")]);

        let plans = plan_import(&incoming, &existing);
        assert_eq!(
            plans,
            vec![ImportPlan::Rename { from: "cht".into(), to: "cht-1".into() }]
        );
    }

    /// A genuinely new profile — different provider AND different name — is
    /// added under its own name.
    #[test]
    fn brand_new_profile_keeps_its_name() {
        let existing = vec![profile("cht", "htmustc", "k1")];
        let incoming = Bundle::new(vec![profile("co-other", "otherprov", "k9")]);

        let plans = plan_import(&incoming, &existing);
        assert_eq!(plans, vec![ImportPlan::Add { alias: "co-other".into() }]);
    }

    /// Same identity + same key under a DIFFERENT name is still identical —
    /// nothing to install, so skip. Guards against importing a bundle whose
    /// aliases were renamed on the other machine.
    #[test]
    fn identical_under_different_name_is_still_skipped() {
        let existing = vec![profile("cht", "htmustc", "k1")];
        let incoming = Bundle::new(vec![profile("claude-ht", "htmustc", "k1")]);

        let plans = plan_import(&incoming, &existing);
        assert_eq!(plans, vec![ImportPlan::Skip { alias: "claude-ht".into() }]);
    }

    /// A generated alias must be typeable at a shell prompt.
    #[test]
    fn generated_alias_is_shell_safe() {
        let taken = vec!["cht".to_string()];
        let out = next_free_alias("cht", &taken);
        assert_eq!(out, "cht-1");
        assert!(!out.contains(' '));
        assert!(!out.contains('('));
    }

    #[test]
    fn suffix_climbs_past_existing_suffixes() {
        let taken = vec!["cht".into(), "cht-1".into(), "cht-2".into()];
        assert_eq!(next_free_alias("cht", &taken), "cht-3");
    }

    /// Two incoming profiles colliding on one free name must not both take it.
    #[test]
    fn collisions_within_one_bundle_do_not_reuse_a_name() {
        let existing = vec![profile("cht", "htmustc", "k1")];
        let incoming = Bundle::new(vec![
            profile("cht", "provA", "k2"),
            profile("cht", "provB", "k3"),
        ]);

        let plans = plan_import(&incoming, &existing);
        assert_eq!(
            plans,
            vec![
                ImportPlan::Rename { from: "cht".into(), to: "cht-1".into() },
                ImportPlan::Rename { from: "cht".into(), to: "cht-2".into() },
            ]
        );
    }
}
