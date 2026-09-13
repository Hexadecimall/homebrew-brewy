use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Hash, Serialize)]
pub enum PackageKind {
    Formula,
    Cask,
}

impl PackageKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Formula => "formula",
            Self::Cask => "cask",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Package {
    pub name: String,
    pub kind: PackageKind,
    pub installed: bool,
    pub outdated: bool,
    pub pinned: bool,
    pub installed_version: Option<String>,
    pub latest_version: Option<String>,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub license: Option<String>,
}

impl Package {
    pub fn new(name: String, kind: PackageKind) -> Self {
        Self {
            name,
            kind,
            installed: false,
            outdated: false,
            pinned: false,
            installed_version: None,
            latest_version: None,
            description: None,
            homepage: None,
            license: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    Install { name: String, cask: bool },
    Uninstall { name: String, cask: bool },
    Upgrade { name: String, cask: bool },
    Pin { name: String },
    Unpin { name: String },
    Update,
    UpgradeAll,
}

impl Operation {
    pub fn package_name(&self) -> Option<&str> {
        match self {
            Self::Install { name, .. }
            | Self::Uninstall { name, .. }
            | Self::Upgrade { name, .. }
            | Self::Pin { name }
            | Self::Unpin { name } => Some(name),
            Self::Update | Self::UpgradeAll => None,
        }
    }

    pub fn verb(&self) -> &'static str {
        match self {
            Self::Install { .. } => "install",
            Self::Uninstall { .. } => "uninstall",
            Self::Upgrade { .. } => "upgrade",
            Self::Pin { .. } => "pin",
            Self::Unpin { .. } => "unpin",
            Self::Update => "update metadata",
            Self::UpgradeAll => "upgrade all packages",
        }
    }

    pub fn command(&self) -> Vec<String> {
        match self {
            Self::Install { name, cask } => package_command("install", name, *cask),
            Self::Uninstall { name, cask } => package_command("uninstall", name, *cask),
            Self::Upgrade { name, cask } => package_command("upgrade", name, *cask),
            Self::Pin { name } => vec!["pin".into(), "--".into(), name.clone()],
            Self::Unpin { name } => vec!["unpin".into(), "--".into(), name.clone()],
            Self::Update => vec!["update".into()],
            Self::UpgradeAll => vec!["upgrade".into()],
        }
    }

    pub fn summary(&self) -> String {
        self.package_name().map_or_else(
            || self.verb().to_string(),
            |name| format!("{} {name}", self.verb()),
        )
    }
}

fn package_command(verb: &str, name: &str, cask: bool) -> Vec<String> {
    let mut args = vec![verb.into()];
    if cask {
        args.push("--cask".into());
    }
    args.push("--".into());
    args.push(name.into());
    args
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Catalog {
    pub packages: Vec<Package>,
    pub taps: Vec<String>,
}

impl Catalog {
    pub fn from_outputs(
        formulae: &str,
        casks: &str,
        installed_json: &str,
        outdated_json: &str,
        taps: &str,
    ) -> Result<Self, String> {
        let installed: BrewJson = serde_json::from_str(installed_json)
            .map_err(|error| format!("cannot parse installed packages: {error}"))?;
        let outdated: BrewJson = serde_json::from_str(outdated_json).unwrap_or_default();
        let mut by_key: HashMap<(PackageKind, String), Package> = HashMap::new();

        for name in formulae
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            let package = Package::new(name.to_string(), PackageKind::Formula);
            by_key.insert((package.kind, package.name.clone()), package);
        }
        for name in casks.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let package = Package::new(name.to_string(), PackageKind::Cask);
            by_key.insert((package.kind, package.name.clone()), package);
        }

        apply_items(&mut by_key, installed.formulae, PackageKind::Formula, true);
        apply_items(&mut by_key, installed.casks, PackageKind::Cask, true);
        apply_items(&mut by_key, outdated.formulae, PackageKind::Formula, false);
        apply_items(&mut by_key, outdated.casks, PackageKind::Cask, false);

        let mut packages: Vec<_> = by_key.into_values().collect();
        packages.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        Ok(Self {
            packages,
            taps: taps
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
        })
    }

    pub fn installed_count(&self) -> usize {
        self.packages
            .iter()
            .filter(|package| package.installed)
            .count()
    }

    pub fn outdated_count(&self) -> usize {
        self.packages
            .iter()
            .filter(|package| package.outdated)
            .count()
    }

    pub fn package_from_json(json: &str) -> Result<Package, String> {
        let parsed: BrewJson = serde_json::from_str(json)
            .map_err(|error| format!("cannot parse package details: {error}"))?;
        let mut packages = HashMap::new();
        if let Some(item) = parsed.formulae.into_iter().next() {
            apply_items(&mut packages, vec![item], PackageKind::Formula, true);
        } else if let Some(item) = parsed.casks.into_iter().next() {
            apply_items(&mut packages, vec![item], PackageKind::Cask, true);
        }
        let mut package = packages
            .into_values()
            .next()
            .ok_or_else(|| "Homebrew returned no package details".to_string())?;
        package.installed = package.installed_version.is_some();
        Ok(package)
    }

    pub fn apply_metadata_outputs(
        &mut self,
        formula_json: &str,
        cask_json: &str,
    ) -> Result<(), String> {
        let formulae: Vec<BrewItem> = serde_json::from_str(formula_json)
            .map_err(|error| format!("cannot parse formula metadata: {error}"))?;
        let casks: Vec<BrewItem> = serde_json::from_str(cask_json)
            .map_err(|error| format!("cannot parse cask metadata: {error}"))?;
        apply_metadata_items(&mut self.packages, formulae, PackageKind::Formula);
        apply_metadata_items(&mut self.packages, casks, PackageKind::Cask);
        self.packages.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        Ok(())
    }

    pub fn merge_cached_metadata(&mut self, cached: &Self) {
        let positions: HashMap<_, _> = self
            .packages
            .iter()
            .enumerate()
            .map(|(index, package)| ((package.kind, package.name.clone()), index))
            .collect();
        for cached_package in &cached.packages {
            if let Some(index) = positions
                .get(&(cached_package.kind, cached_package.name.clone()))
                .copied()
            {
                let package = &mut self.packages[index];
                fill_missing(&mut package.description, &cached_package.description);
                fill_missing(&mut package.homepage, &cached_package.homepage);
                fill_missing(&mut package.license, &cached_package.license);
                fill_missing(&mut package.latest_version, &cached_package.latest_version);
            } else {
                let mut package = cached_package.clone();
                package.installed = false;
                package.outdated = false;
                package.pinned = false;
                package.installed_version = None;
                self.packages.push(package);
            }
        }
        self.packages.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    }
}

fn fill_missing(target: &mut Option<String>, source: &Option<String>) {
    if target.is_none() {
        target.clone_from(source);
    }
}

fn apply_metadata_items(packages: &mut Vec<Package>, items: Vec<BrewItem>, kind: PackageKind) {
    let mut positions: HashMap<(PackageKind, String), usize> = packages
        .iter()
        .enumerate()
        .map(|(index, package)| ((package.kind, package.name.clone()), index))
        .collect();
    for item in items {
        let name = item
            .token
            .or_else(|| item.name.and_then(OneOrMany::first))
            .or(item.full_name)
            .unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let index = *positions.entry((kind, name.clone())).or_insert_with(|| {
            packages.push(Package::new(name, kind));
            packages.len() - 1
        });
        let package = &mut packages[index];
        package.description = Some(
            item.desc
                .unwrap_or_else(|| "No description provided by Homebrew".into()),
        );
        if item.homepage.is_some() {
            package.homepage = item.homepage;
        }
        if item.license.is_some() {
            package.license = item.license;
        }
        let version = item
            .current_version
            .or_else(|| item.versions.and_then(|versions| versions.stable))
            .or(item.version);
        if version.is_some() {
            package.latest_version = version;
        }
    }
}

fn apply_items(
    packages: &mut HashMap<(PackageKind, String), Package>,
    items: Vec<BrewItem>,
    kind: PackageKind,
    installed: bool,
) {
    for item in items {
        let name = item
            .token
            .or_else(|| item.name.and_then(OneOrMany::first))
            .or(item.full_name)
            .unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let package = packages
            .entry((kind, name.clone()))
            .or_insert_with(|| Package::new(name, kind));
        package.description = item.desc.or(package.description.take());
        package.homepage = item.homepage.or(package.homepage.take());
        package.license = item.license.or(package.license.take());
        package.pinned |= item.pinned;
        package.outdated |= item.outdated || !installed;
        if installed {
            package.installed = true;
            package.installed_version = item.installed.as_ref().and_then(InstalledField::version);
        }
        package.latest_version = item
            .current_version
            .or_else(|| item.versions.and_then(|versions| versions.stable))
            .or(item.version)
            .or(package.latest_version.take());
    }
}

#[derive(Default, Deserialize)]
struct BrewJson {
    #[serde(default)]
    formulae: Vec<BrewItem>,
    #[serde(default)]
    casks: Vec<BrewItem>,
}

#[derive(Default, Deserialize)]
struct BrewItem {
    name: Option<OneOrMany>,
    token: Option<String>,
    full_name: Option<String>,
    desc: Option<String>,
    homepage: Option<String>,
    license: Option<String>,
    #[serde(default)]
    installed: Option<InstalledField>,
    versions: Option<Versions>,
    version: Option<String>,
    current_version: Option<String>,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    outdated: bool,
}

#[derive(Deserialize)]
struct InstalledVersion {
    version: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    fn first(self) -> Option<String> {
        match self {
            Self::One(value) => Some(value),
            Self::Many(values) => values.into_iter().next(),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum InstalledField {
    Entries(Vec<InstalledVersion>),
    Version(String),
}

impl InstalledField {
    fn version(&self) -> Option<String> {
        match self {
            Self::Entries(entries) => entries.first().and_then(|entry| entry.version.clone()),
            Self::Version(version) => Some(version.clone()),
        }
    }
}

#[derive(Deserialize)]
struct Versions {
    stable: Option<String>,
}

pub fn fuzzy_score(candidate: &str, query: &str) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    let candidate = candidate.to_ascii_lowercase();
    let query = query.to_ascii_lowercase();
    if candidate == query {
        return Some(10_000);
    }
    if let Some(index) = candidate.find(&query) {
        return Some(5_000 - index as i64 * 8 - candidate.len() as i64);
    }

    let mut chars = query.chars();
    let mut wanted = chars.next()?;
    let mut score = 0_i64;
    let mut previous = None;
    let mut matched = 0;
    for (index, ch) in candidate.chars().enumerate() {
        if ch == wanted {
            score += if previous == Some(index.saturating_sub(1)) {
                30
            } else {
                10
            };
            score -= index as i64;
            previous = Some(index);
            matched += 1;
            if let Some(next) = chars.next() {
                wanted = next;
            } else {
                return Some(score + matched * 20);
            }
        }
    }
    None
}

pub fn operation_for(package: &Package) -> Operation {
    let cask = package.kind == PackageKind::Cask;
    if package.outdated {
        Operation::Upgrade {
            name: package.name.clone(),
            cask,
        }
    } else if package.installed {
        Operation::Uninstall {
            name: package.name.clone(),
            cask,
        }
    } else {
        Operation::Install {
            name: package.name.clone(),
            cask,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_matching_rewards_exact_and_contiguous_matches() {
        assert!(fuzzy_score("ripgrep", "ripgrep") > fuzzy_score("ripgrep", "rg"));
        assert!(fuzzy_score("visual-studio-code", "vsc").is_some());
        assert!(fuzzy_score("wget", "xyz").is_none());
    }

    #[test]
    fn catalog_merges_brew_json_with_available_names() {
        let installed = r#"{"formulae":[{"name":"wget","desc":"Internet file retriever","installed":[{"version":"1.2"}],"versions":{"stable":"1.3"},"outdated":true}],"casks":[{"token":"ghostty","installed":[{"version":"2.0"}]}]}"#;
        let outdated = r#"{"formulae":[{"name":"wget","installed_versions":["1.2"],"current_version":"1.3"}],"casks":[]}"#;
        let catalog = Catalog::from_outputs(
            "wget\nripgrep\n",
            "ghostty\n",
            installed,
            outdated,
            "homebrew/core\n",
        )
        .unwrap();
        assert_eq!(catalog.installed_count(), 2);
        assert_eq!(catalog.outdated_count(), 1);
        assert_eq!(catalog.taps, ["homebrew/core"]);
        let wget = catalog
            .packages
            .iter()
            .find(|package| package.name == "wget")
            .unwrap();
        assert_eq!(wget.description.as_deref(), Some("Internet file retriever"));
        assert_eq!(wget.installed_version.as_deref(), Some("1.2"));
        assert_eq!(wget.latest_version.as_deref(), Some("1.3"));
    }

    #[test]
    fn installed_package_stages_uninstall_unless_outdated() {
        let mut package = Package::new("demo".into(), PackageKind::Formula);
        package.installed = true;
        assert!(matches!(
            operation_for(&package),
            Operation::Uninstall { .. }
        ));
        package.outdated = true;
        assert!(matches!(operation_for(&package), Operation::Upgrade { .. }));
    }

    #[test]
    fn detail_json_does_not_mark_an_available_package_installed() {
        let json = r#"{"formulae":[{"name":"jq","desc":"JSON processor","installed":[],"versions":{"stable":"1.8.2"}}],"casks":[]}"#;
        let package = Catalog::package_from_json(json).unwrap();
        assert_eq!(package.name, "jq");
        assert!(!package.installed);
        assert_eq!(package.latest_version.as_deref(), Some("1.8.2"));
    }

    #[test]
    fn package_names_are_passed_after_an_option_separator() {
        let package = Package::new("--debug".into(), PackageKind::Formula);
        assert_eq!(
            operation_for(&package).command(),
            ["install", "--", "--debug"]
        );
        assert_eq!(
            Operation::Pin {
                name: "--debug".into()
            }
            .command(),
            ["pin", "--", "--debug"]
        );
    }

    #[test]
    fn modern_cask_detail_fields_are_supported() {
        let json = r#"{"formulae":[],"casks":[{"token":"sample-app","name":["Sample App"],"installed":"2.0","version":"2.1"}]}"#;
        let package = Catalog::package_from_json(json).unwrap();
        assert_eq!(package.name, "sample-app");
        assert!(package.installed);
        assert_eq!(package.installed_version.as_deref(), Some("2.0"));
        assert_eq!(package.latest_version.as_deref(), Some("2.1"));
    }

    #[test]
    fn available_cask_version_is_not_treated_as_installed() {
        let json =
            r#"{"formulae":[],"casks":[{"token":"sample-app","installed":null,"version":"2.1"}]}"#;
        let package = Catalog::package_from_json(json).unwrap();
        assert!(!package.installed);
        assert_eq!(package.installed_version, None);
        assert_eq!(package.latest_version.as_deref(), Some("2.1"));
    }

    #[test]
    fn bulk_metadata_populates_descriptions_without_detail_commands() {
        let mut catalog = Catalog::from_outputs(
            "jq\n",
            "ghostty\n",
            r#"{"formulae":[],"casks":[]}"#,
            r#"{"formulae":[],"casks":[]}"#,
            "",
        )
        .unwrap();
        catalog
            .apply_metadata_outputs(
                r#"[{"name":"jq","desc":"JSON processor","homepage":"https://jqlang.org/","license":"MIT","versions":{"stable":"1.8.1"}}]"#,
                r#"[{"token":"ghostty","desc":"Terminal emulator","homepage":"https://ghostty.org/","version":"1.2.3"},{"token":"undocumented","desc":null,"homepage":"https://example.com/","version":"1.0"}]"#,
            )
            .unwrap();

        let jq = catalog
            .packages
            .iter()
            .find(|package| package.name == "jq")
            .unwrap();
        assert_eq!(jq.description.as_deref(), Some("JSON processor"));
        assert_eq!(jq.latest_version.as_deref(), Some("1.8.1"));
        let ghostty = catalog
            .packages
            .iter()
            .find(|package| package.name == "ghostty")
            .unwrap();
        assert_eq!(ghostty.description.as_deref(), Some("Terminal emulator"));
        let undocumented = catalog
            .packages
            .iter()
            .find(|package| package.name == "undocumented")
            .unwrap();
        assert_eq!(
            undocumented.description.as_deref(),
            Some("No description provided by Homebrew")
        );
    }

    #[test]
    fn upgrade_all_runs_homebrew_upgrade() {
        assert_eq!(Operation::UpgradeAll.command(), ["upgrade"]);
    }
}
