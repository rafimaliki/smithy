//! Dependency resolution between add-ons. Pure, so it is tested without a window.
use super::AddonInfo;

/// Ids to switch on, dependencies first, so turning `id` on also turns on what it
/// needs. Already-enabled ids are not repeated.
pub fn resolve_enable(id: &str, enabled: &[String], infos: &[AddonInfo]) -> Vec<&'static str> {
    let mut out = Vec::new();
    visit(id, enabled, infos, &mut out);
    out
}

fn visit(id: &str, enabled: &[String], infos: &[AddonInfo], out: &mut Vec<&'static str>) {
    let Some(info) = infos.iter().find(|i| i.id == id) else {
        return;
    };
    if enabled.iter().any(|e| e == id) || out.contains(&info.id) {
        return;
    }
    for dep in info.requires {
        visit(dep, enabled, infos, out);
    }
    out.push(info.id);
}

/// Ids to switch off, dependents first, so turning `id` off also turns off what
/// needs it.
pub fn resolve_disable(id: &str, enabled: &[String], infos: &[AddonInfo]) -> Vec<&'static str> {
    let mut out = Vec::new();
    for info in infos {
        if info.requires.contains(&id) && enabled.iter().any(|e| e == info.id) {
            for dependent in resolve_disable(info.id, enabled, infos) {
                if !out.contains(&dependent) {
                    out.push(dependent);
                }
            }
        }
    }
    if let Some(info) = infos.iter().find(|i| i.id == id) {
        if enabled.iter().any(|e| e == id) && !out.contains(&info.id) {
            out.push(info.id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const INFOS: [AddonInfo; 2] = [
        AddonInfo {
            id: "git",
            name: "",
            description: "",
            requires: &[],
        },
        AddonInfo {
            id: "pr",
            name: "",
            description: "",
            requires: &["git"],
        },
    ];

    fn on(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn enabling_pr_pulls_in_git_first() {
        assert_eq!(resolve_enable("pr", &on(&[]), &INFOS), ["git", "pr"]);
        assert_eq!(resolve_enable("pr", &on(&["git"]), &INFOS), ["pr"]);
    }

    #[test]
    fn disabling_git_turns_off_pr_first() {
        assert_eq!(
            resolve_disable("git", &on(&["git", "pr"]), &INFOS),
            ["pr", "git"]
        );
        assert_eq!(resolve_disable("git", &on(&["git"]), &INFOS), ["git"]);
    }
}
