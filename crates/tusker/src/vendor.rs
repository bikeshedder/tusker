//! Vendoring: copying the migrations of a dependency into the migration
//! folder of the project that uses it.
//!
//! A dependency is a crate that ships its own schema files and its own linear
//! history of migrations next to its `Cargo.toml`, configured by its own
//! `tusker.toml`. The project lists the crates it vendors:
//!
//! ```toml
//! [vendor.tragwerk]
//! ```
//!
//! `tusker migration vendor` copies every migration of those crates that the
//! project's folder does not contain yet to the end of it. Each copy starts
//! with a header that says where it came from:
//!
//! ```sql
//! -- tusker:vendored
//! --   crate: tragwerk 0.3.0
//! --   source: 0002_auth_init.sql
//! --   sha256: 3f9a…
//! --
//! ```
//!
//! A vendored crate's own migrations must not be vendored from yet another
//! crate: one crate building on another one's migrations is not supported.
//!
//! The runner does not know about any of this: a vendored migration is an
//! ordinary migration whose header is a comment. Once committed, it keeps its
//! place in the project's history, so the order in which migrations run is
//! fixed by the project and never computed from dependencies.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tusker_migration::NO_TRANSACTION_MARKER;

use crate::config::{Config, MigrationsConfig, SchemaConfig};

/// First line of the header of a vendored migration.
pub(crate) const VENDORED_MARKER: &str = "-- tusker:vendored";

/// `[vendor.<crate>]` in the project's `tusker.toml`. The section name is the
/// name of the crate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VendorConfig {
    /// Which version to pick when the dependency graph contains more than
    /// one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<String>,
    /// A directory containing the crate's `tusker.toml`, instead of looking
    /// the crate up through `cargo metadata`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
}

/// The parts of a dependency's `tusker.toml` that vendoring reads.
#[derive(Debug, Default, Deserialize)]
struct DependencyConfig {
    #[serde(default)]
    schema: SchemaConfig,
    #[serde(default)]
    migrations: MigrationsConfig,
}

/// Lowercase hex SHA-256 of the given bytes.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

/// The header of a vendored migration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Header {
    /// Name of the crate the migration comes from.
    pub(crate) dependency: String,
    /// Version of the crate the copy was taken from, if known.
    pub(crate) version: Option<String>,
    /// File name of the migration in the crate.
    pub(crate) source: String,
    /// SHA-256 of the original file.
    pub(crate) sha256: String,
}

impl Header {
    fn key(&self) -> Key {
        (self.dependency.clone(), self.source.clone())
    }
}

/// A migration by its origin: crate name and file name in that crate.
type Key = (String, String);

/// Renders a vendored migration: the original with the header inserted. A
/// leading `-- tusker:no-transaction` stays the first line, because the
/// runner only looks there.
pub(crate) fn render_vendored(header: &Header, original: &str) -> String {
    let split = match original.split_inclusive('\n').next() {
        Some(first) if first.trim() == NO_TRANSACTION_MARKER => first.len(),
        _ => 0,
    };
    let (first, rest) = original.split_at(split);
    let first = if !first.is_empty() && !first.ends_with('\n') {
        format!("{first}\n")
    } else {
        first.to_owned()
    };
    let crate_line = match &header.version {
        Some(version) => format!("{} {}", header.dependency, version),
        None => header.dependency.clone(),
    };
    format!(
        "{first}{VENDORED_MARKER}\n--   crate: {crate_line}\n--   source: {}\n--   sha256: {}\n--\n{rest}",
        header.source, header.sha256,
    )
}

/// Splits a vendored migration into its header and the original file.
/// Returns `None` for a migration that is not vendored.
pub(crate) fn parse_vendored(sql: &str) -> Result<Option<(Header, String)>> {
    let lines: Vec<&str> = sql.split_inclusive('\n').collect();
    let start = match lines.first() {
        Some(first) if first.trim() == NO_TRANSACTION_MARKER => 1,
        _ => 0,
    };
    if lines.get(start).map(|l| l.trim_end()) != Some(VENDORED_MARKER) {
        return Ok(None);
    }
    let mut fields = HashMap::new();
    let mut end = None;
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        let line = line.trim_end();
        if line == "--" {
            end = Some(i);
            break;
        }
        let field = line
            .strip_prefix("--")
            .and_then(|rest| rest.trim_start().split_once(':'))
            .ok_or_else(|| anyhow!("Unexpected line in the vendored header: {line:?}"))?;
        let _ = fields.insert(field.0.trim().to_owned(), field.1.trim().to_owned());
    }
    let end = end.ok_or_else(|| anyhow!("The vendored header does not end with a `--` line"))?;
    let mut take = |key: &str| {
        fields
            .remove(key)
            .ok_or_else(|| anyhow!("The vendored header has no `{key}`"))
    };
    let crate_line = take("crate")?;
    let source = take("source")?;
    let sha256 = take("sha256")?;
    let (dependency, version) = match crate_line.split_once(' ') {
        Some((name, version)) => (name.to_owned(), Some(version.trim().to_owned())),
        None => (crate_line, None),
    };
    let original = lines[..start].concat() + &lines[end + 1..].concat();
    Ok(Some((
        Header {
            dependency,
            version,
            source,
            sha256,
        },
        original,
    )))
}

/// Splits a `<number>_<name>` file stem.
fn parse_stem(stem: &str) -> Result<(i32, String)> {
    let (number, name) = stem.split_once('_').unwrap_or((stem, ""));
    let number = number
        .parse()
        .with_context(|| format!("Expected <number>_<name>: {stem:?}"))?;
    Ok((number, name.to_owned()))
}

/// The `.sql` files matching a glob, sorted by path.
fn glob_sql(pattern: &str) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for path in glob::glob(pattern).with_context(|| format!("Invalid glob {pattern:?}"))? {
        let path = path?;
        if path.extension().is_some_and(|ext| ext == "sql") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A migration of a dependency.
#[derive(Debug, Clone)]
pub(crate) struct SourceMigration {
    /// The file stem after the number, e.g. `init`.
    pub(crate) name: String,
    pub(crate) file_name: String,
    pub(crate) sql: String,
    pub(crate) sha256: String,
}

/// A crate with its migrations.
#[derive(Debug)]
pub(crate) struct Dependency {
    /// The crate name (the `[vendor.<name>]` key).
    pub(crate) name: String,
    pub(crate) version: Option<String>,
    /// The crate's schema glob as an absolute pattern.
    pub(crate) schema: String,
    /// The crate's migrations in order.
    pub(crate) migrations: Vec<SourceMigration>,
}

impl Dependency {
    /// Locates and loads a dependency.
    fn load(name: &str, cfg: &VendorConfig, cargo: &mut Cargo) -> Result<Self> {
        let (dir, version) = match &cfg.path {
            Some(path) => {
                let dir = cargo.base_dir.join(path);
                let version = cargo_package_version(&dir);
                (dir, version)
            }
            None => {
                let (dir, version) = find_crate(cargo.metadata()?, name, cfg.version.as_deref())
                    .with_context(|| format!("[vendor.{name}]: locating the crate"))?;
                (dir, Some(version))
            }
        };
        Self::load_dir(name, &dir, version)
    }

    fn load_dir(name: &str, dir: &Path, version: Option<String>) -> Result<Self> {
        let config_path = dir.join("tusker.toml");
        let config: DependencyConfig = toml::from_str(
            &fs::read_to_string(&config_path)
                .with_context(|| format!("Reading {}", config_path.display()))?,
        )
        .with_context(|| format!("Parsing {}", config_path.display()))?;
        let pattern = dir.join(&config.migrations.filename);
        let mut numbered = Vec::new();
        for path in glob_sql(&pattern.to_string_lossy())? {
            let (number, stem_name) = parse_stem(&file_stem(&path))
                .with_context(|| format!("Migration of {name}: {}", path.display()))?;
            let bytes = fs::read(&path)?;
            let sql = String::from_utf8(bytes)
                .with_context(|| format!("{} is not UTF-8", path.display()))?;
            if let Some((header, _)) =
                parse_vendored(&sql).with_context(|| format!("In {}", path.display()))?
            {
                bail!(
                    "{} is vendored from {}: tusker cannot vendor a crate whose migrations \
                     are vendored from another one",
                    path.display(),
                    header.dependency
                );
            }
            numbered.push((
                number,
                SourceMigration {
                    name: stem_name,
                    file_name: file_name(&path),
                    sha256: sha256_hex(sql.as_bytes()),
                    sql,
                },
            ));
        }
        numbered.sort_by_key(|(number, _)| *number);
        if let Some(pair) = numbered.windows(2).find(|w| w[0].0 == w[1].0) {
            bail!(
                "{name} has two migrations numbered {}: {} and {}",
                pair[0].0,
                pair[0].1.file_name,
                pair[1].1.file_name
            );
        }
        Ok(Self {
            name: name.to_owned(),
            version,
            schema: dir
                .join(&config.schema.filename)
                .to_string_lossy()
                .into_owned(),
            migrations: numbered.into_iter().map(|(_, m)| m).collect(),
        })
    }

    /// The position and migration of a file of the crate.
    fn find(&self, file: &str) -> Option<(usize, &SourceMigration)> {
        self.migrations
            .iter()
            .enumerate()
            .find(|(_, m)| m.file_name == file)
    }
}

/// The `package.version` of the `Cargo.toml` in `dir`, if there is one.
fn cargo_package_version(dir: &Path) -> Option<String> {
    let manifest = fs::read_to_string(dir.join("Cargo.toml")).ok()?;
    let manifest: toml::Value = toml::from_str(&manifest).ok()?;
    manifest
        .get("package")?
        .get("version")?
        .as_str()
        .map(str::to_owned)
}

/// `cargo metadata` of the project, run once when first needed.
struct Cargo {
    base_dir: PathBuf,
    metadata: Option<serde_json::Value>,
}

impl Cargo {
    fn new(base_dir: &Path) -> Self {
        Self {
            base_dir: base_dir.to_owned(),
            metadata: None,
        }
    }

    fn metadata(&mut self) -> Result<&serde_json::Value> {
        if self.metadata.is_none() {
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
            let output = Command::new(cargo)
                .args(["metadata", "--format-version", "1"])
                .current_dir(&self.base_dir)
                .output()
                .context("Running `cargo metadata`")?;
            if !output.status.success() {
                bail!(
                    "`cargo metadata` failed:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            self.metadata =
                Some(serde_json::from_slice(&output.stdout).context("Parsing `cargo metadata`")?);
        }
        Ok(self.metadata.as_ref().expect("loaded above"))
    }
}

/// Picks a package out of `cargo metadata` output.
fn find_crate(
    metadata: &serde_json::Value,
    krate: &str,
    version: Option<&str>,
) -> Result<(PathBuf, String)> {
    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| anyhow!("`cargo metadata` lists no packages"))?;
    let candidates: Vec<(PathBuf, String)> = packages
        .iter()
        .filter(|p| p["name"] == krate)
        .filter_map(|p| {
            let manifest = PathBuf::from(p["manifest_path"].as_str()?);
            Some((
                manifest.parent()?.to_owned(),
                p["version"].as_str()?.to_owned(),
            ))
        })
        .filter(|(_, v)| version.is_none_or(|wanted| v == wanted))
        .collect();
    match candidates.len() {
        0 => match version {
            Some(version) => bail!("The dependency graph has no {krate} {version}"),
            None => bail!("The dependency graph has no crate {krate:?}; add it to Cargo.toml"),
        },
        1 => Ok(candidates.into_iter().next().expect("one candidate")),
        _ => bail!(
            "The dependency graph has several versions of {krate} ({}); pick one with `version`",
            candidates
                .iter()
                .map(|(_, v)| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// The crates the package in `root_dir` depends on, directly or not, through
/// normal dependencies: name and directory. Empty when no package of the
/// metadata lives in `root_dir`.
fn reachable_crates(metadata: &serde_json::Value, root_dir: &Path) -> Vec<(String, PathBuf)> {
    let dir_of = |p: &serde_json::Value| {
        p["manifest_path"]
            .as_str()
            .and_then(|m| Path::new(m).parent().map(Path::to_owned))
    };
    let empty = Vec::new();
    let packages = metadata["packages"].as_array().unwrap_or(&empty);
    let Some(root) = packages
        .iter()
        .find(|p| dir_of(p).as_deref() == Some(root_dir))
        .and_then(|p| p["id"].as_str())
    else {
        return Vec::new();
    };
    let nodes: HashMap<&str, &serde_json::Value> = metadata["resolve"]["nodes"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|n| Some((n["id"].as_str()?, n)))
        .collect();
    let mut reached = BTreeSet::new();
    let mut todo = vec![root];
    while let Some(id) = todo.pop() {
        let Some(node) = nodes.get(id) else { continue };
        for dep in node["deps"].as_array().unwrap_or(&empty) {
            let normal = dep["dep_kinds"]
                .as_array()
                .unwrap_or(&empty)
                .iter()
                .any(|k| k["kind"].is_null());
            if let Some(pkg) = dep["pkg"].as_str().filter(|_| normal) {
                if reached.insert(pkg) {
                    todo.push(pkg);
                }
            }
        }
    }
    packages
        .iter()
        .filter(|p| p["id"].as_str().is_some_and(|id| reached.contains(id)))
        .filter_map(|p| Some((p["name"].as_str()?.to_owned(), dir_of(p)?)))
        .collect()
}

/// Whether the crate in `dir` ships migrations.
fn ships_migrations(dir: &Path) -> bool {
    dir.join("tusker.toml").is_file()
        && Dependency::load_dir("", dir, None).map_or(true, |d| !d.migrations.is_empty())
}

/// A migration in the project's own folder.
#[derive(Debug)]
pub(crate) struct ProjectMigration {
    pub(crate) number: i32,
    pub(crate) path: PathBuf,
    pub(crate) vendored: Option<(Header, String)>,
}

/// Loads the project's migrations, sorted by number.
pub(crate) fn load_project_migrations(pattern: &str) -> Result<Vec<ProjectMigration>> {
    let mut migrations = Vec::new();
    for path in glob_sql(pattern)? {
        let (number, _) = parse_stem(&file_stem(&path))?;
        let sql =
            fs::read_to_string(&path).with_context(|| format!("Reading {}", path.display()))?;
        let vendored = parse_vendored(&sql).with_context(|| format!("In {}", path.display()))?;
        migrations.push(ProjectMigration {
            number,
            path,
            vendored,
        });
    }
    migrations.sort_by_key(|m| m.number);
    Ok(migrations)
}

/// Something wrong with the vendored migrations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Problem {
    /// A crate in the dependency graph ships migrations, but has no
    /// `[vendor.<crate>]` section.
    Unlisted { krate: String },
    /// A copy was edited: its body no longer matches its own hash.
    EditedCopy { file: String },
    /// The dependency's file changed after it was vendored.
    ChangedUpstream { file: String, source: String },
    /// A vendored migration comes before an earlier one of its crate.
    Order { file: String, missing: String },
    /// A header points to something the configuration does not know.
    Unknown { file: String, reason: String },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unlisted { krate } => write!(
                f,
                "{krate} is in the dependency graph and ships migrations; \
                 add [vendor.{krate}] to tusker.toml"
            ),
            Self::EditedCopy { file } => write!(
                f,
                "{file}: edited – its body no longer matches the sha256 in its header. \
                 Vendored migrations are not edited; change the dependency instead."
            ),
            Self::ChangedUpstream { file, source } => write!(
                f,
                "{file}: the dependency's {source} changed after it was vendored. \
                 Released migrations of a dependency must not be edited."
            ),
            Self::Order { file, missing } => write!(
                f,
                "{file}: comes before {missing}. Vendored migrations keep the order of \
                 the crate they come from."
            ),
            Self::Unknown { file, reason } => write!(f, "{file}: {reason}"),
        }
    }
}

/// A migration to vendor.
#[derive(Debug, Clone)]
pub(crate) struct Pending {
    pub(crate) dependency: String,
    pub(crate) version: Option<String>,
    pub(crate) migration: SourceMigration,
}

/// The result of comparing the project's folder with its dependencies.
#[derive(Debug, Default)]
pub(crate) struct Report {
    pub(crate) problems: Vec<Problem>,
    /// Migrations to vendor, in the order to append them.
    pub(crate) pending: Vec<Pending>,
}

impl Report {
    pub(crate) fn is_clean(&self) -> bool {
        self.problems.is_empty() && self.pending.is_empty()
    }

    /// Prints the problems and pending migrations to stderr.
    pub(crate) fn print(&self) {
        for problem in &self.problems {
            eprintln!("error: {problem}");
        }
        if !self.pending.is_empty() {
            eprintln!("Not vendored yet – run `tusker migration vendor`:");
            for pending in &self.pending {
                eprintln!("  {}: {}", pending.dependency, pending.migration.file_name);
            }
        }
    }
}

/// Compares the project's migrations with its dependencies.
pub(crate) fn analyze(dependencies: &[Dependency], project: &[ProjectMigration]) -> Report {
    let mut report = Report::default();
    let by_name: BTreeMap<&str, &Dependency> =
        dependencies.iter().map(|d| (d.name.as_str(), d)).collect();

    // The project's copies, in the project's order.
    let mut seen: BTreeSet<Key> = BTreeSet::new();
    for migration in project {
        let Some((header, original)) = &migration.vendored else {
            continue;
        };
        let file = file_name(&migration.path);
        if sha256_hex(original.as_bytes()) != header.sha256 {
            report
                .problems
                .push(Problem::EditedCopy { file: file.clone() });
        }
        let Some(dependency) = by_name.get(header.dependency.as_str()) else {
            report.problems.push(Problem::Unknown {
                file,
                reason: format!(
                    "vendored from {}, which has no [vendor.{}] section",
                    header.dependency, header.dependency
                ),
            });
            continue;
        };
        let Some((index, source)) = dependency.find(&header.source) else {
            report.problems.push(Problem::Unknown {
                file,
                reason: format!("{} has no migration {}", dependency.name, header.source),
            });
            continue;
        };
        if source.sha256 != header.sha256 {
            report.problems.push(Problem::ChangedUpstream {
                file: file.clone(),
                source: source.file_name.clone(),
            });
        }
        if !seen.insert(header.key()) {
            report.problems.push(Problem::Unknown {
                file: file.clone(),
                reason: format!("{} is vendored more than once", source.file_name),
            });
        }
        // The crate's earlier migrations must already be in place.
        if let Some(missing) = dependency.migrations[..index]
            .iter()
            .find(|m| !seen.contains(&(dependency.name.clone(), m.file_name.clone())))
        {
            report.problems.push(Problem::Order {
                file,
                missing: format!("{} {}", dependency.name, missing.file_name),
            });
        }
    }

    // What is missing, each crate's in its order.
    for dependency in dependencies {
        for migration in &dependency.migrations {
            if !seen.contains(&(dependency.name.clone(), migration.file_name.clone())) {
                report.pending.push(Pending {
                    dependency: dependency.name.clone(),
                    version: dependency.version.clone(),
                    migration: migration.clone(),
                });
            }
        }
    }
    report
}

/// The directory a migrations glob points into: everything before the first
/// component with a wildcard.
pub(crate) fn glob_base(pattern: &str) -> PathBuf {
    Path::new(pattern)
        .components()
        .take_while(|c| !c.as_os_str().to_string_lossy().contains(['*', '?', '[']))
        .collect()
}

/// Writes the pending migrations to the end of the project's folder and
/// returns the paths written.
pub(crate) fn write_pending(
    report: &Report,
    project: &[ProjectMigration],
    dir: &Path,
) -> Result<Vec<PathBuf>> {
    let first = project.last().map_or(1, |m| m.number + 1);
    let mut written = Vec::new();
    fs::create_dir_all(dir)?;
    for (next, pending) in (first..).zip(&report.pending) {
        let header = Header {
            dependency: pending.dependency.clone(),
            version: pending.version.clone(),
            source: pending.migration.file_name.clone(),
            sha256: pending.migration.sha256.clone(),
        };
        let prefix = pending.dependency.replace('-', "_");
        let path = dir.join(format!("{next:04}_{prefix}_{}.sql", pending.migration.name));
        if path.exists() {
            bail!("{} exists already", path.display());
        }
        fs::write(&path, render_vendored(&header, &pending.migration.sql))?;
        written.push(path);
    }
    Ok(written)
}

/// The project's dependencies and what `cargo metadata` found about them.
pub(crate) struct Vendored {
    pub(crate) dependencies: Vec<Dependency>,
    /// Crates in the dependency graph that ship migrations but are not
    /// listed.
    pub(crate) unlisted: Vec<String>,
}

/// Loads all `[vendor.*]` dependencies of a project. When one of them is
/// looked up through `cargo metadata`, or the project is a Cargo package, the
/// dependency graph is also searched for crates that ship migrations without
/// being listed – a forgotten `[vendor.<crate>]` is caught.
pub(crate) fn load(vendor: &BTreeMap<String, VendorConfig>, base_dir: &Path) -> Result<Vendored> {
    let mut cargo = Cargo::new(base_dir);
    let dependencies = vendor
        .iter()
        .map(|(name, cfg)| Dependency::load(name, cfg, &mut cargo))
        .collect::<Result<Vec<_>>>()?;
    if cargo.metadata.is_none() && base_dir.join("Cargo.toml").is_file() {
        if let Err(error) = cargo.metadata() {
            eprintln!(
                "warning: {error:#}\nCrates that ship migrations without a [vendor.<crate>] section are not checked."
            );
        }
    }
    let unlisted = match &cargo.metadata {
        Some(metadata) => {
            let root = base_dir
                .canonicalize()
                .unwrap_or_else(|_| base_dir.to_owned());
            reachable_crates(metadata, &root)
                .into_iter()
                .filter(|(name, _)| !vendor.contains_key(name))
                .filter(|(_, dir)| ships_migrations(dir))
                .map(|(name, _)| name)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        }
        None => Vec::new(),
    };
    Ok(Vendored {
        dependencies,
        unlisted,
    })
}

impl Vendored {
    /// Compares the project's migrations with the dependencies.
    pub(crate) fn analyze(&self, project: &[ProjectMigration]) -> Result<Report> {
        let mut report = analyze(&self.dependencies, project);
        let mut problems: Vec<Problem> = self
            .unlisted
            .iter()
            .map(|krate| Problem::Unlisted {
                krate: krate.clone(),
            })
            .collect();
        problems.append(&mut report.problems);
        report.problems = problems;
        Ok(report)
    }
}

/// The schema globs of a project: its dependencies' schema files, then its
/// own `[schema] filename`.
pub(crate) fn project_schema_globs(cfg: &Config, base_dir: &Path) -> Result<Vec<String>> {
    let vendored = load(&cfg.vendor, base_dir)?;
    let mut globs: Vec<String> = vendored
        .dependencies
        .iter()
        .map(|d| d.schema.clone())
        .collect();
    globs.push(cfg.schema.filename.clone());
    Ok(globs)
}

/// Compares the vendored migrations with the dependencies. Prints what is
/// wrong or pending and returns `false` unless everything is in place.
pub(crate) fn verify(cfg: &Config, base_dir: &Path) -> Result<bool> {
    if cfg.vendor.is_empty() && !base_dir.join("Cargo.toml").is_file() {
        return Ok(true);
    }
    let project = load_project_migrations(&cfg.migrations.filename)?;
    let report = load(&cfg.vendor, base_dir)?.analyze(&project)?;
    report.print();
    Ok(report.is_clean())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> Header {
        Header {
            dependency: "tragwerk".into(),
            version: Some("0.3.0".into()),
            source: "0004_auth_add_x.sql".into(),
            sha256: "abc".into(),
        }
    }

    #[test]
    fn render_and_parse_round_trip() {
        let original = "-- Adds x\nALTER TABLE x ADD y int;\n";
        let rendered = render_vendored(&header(), original);
        assert!(rendered.starts_with(VENDORED_MARKER));
        let (parsed, body) = parse_vendored(&rendered).unwrap().unwrap();
        assert_eq!(parsed, header());
        assert_eq!(body, original);
    }

    #[test]
    fn no_transaction_stays_first() {
        let original = "-- tusker:no-transaction\nCREATE INDEX CONCURRENTLY i ON t (c);\n";
        let rendered = render_vendored(&header(), original);
        assert!(rendered.starts_with("-- tusker:no-transaction\n-- tusker:vendored\n"));
        let migration = tusker_migration::Migration::new(1, "x", rendered.clone());
        assert!(migration.no_transaction());
        let (_, body) = parse_vendored(&rendered).unwrap().unwrap();
        assert_eq!(body, original);
    }

    #[test]
    fn plain_migration_is_not_vendored() {
        assert!(parse_vendored("CREATE TABLE x ();\n").unwrap().is_none());
        assert!(parse_vendored("").unwrap().is_none());
    }

    #[test]
    fn version_is_optional() {
        let header = Header {
            version: None,
            ..header()
        };
        let rendered = render_vendored(&header, "SELECT 1;\n");
        assert_eq!(parse_vendored(&rendered).unwrap().unwrap().0, header);
    }

    #[test]
    fn unterminated_header_is_an_error() {
        assert!(parse_vendored("-- tusker:vendored\n--   crate: x\nSELECT 1;\n").is_err());
    }

    #[test]
    fn glob_base_stops_at_the_first_wildcard() {
        assert_eq!(
            glob_base("db/migrations/**/*.sql"),
            PathBuf::from("db/migrations")
        );
        assert_eq!(glob_base("migrations/*.sql"), PathBuf::from("migrations"));
    }

    #[test]
    fn find_crate_picks_the_package() {
        let metadata = serde_json::json!({"packages": [
            {"name": "mylib", "version": "0.3.0", "manifest_path": "/deps/core/Cargo.toml"},
            {"name": "other", "version": "1.0.0", "manifest_path": "/deps/other/Cargo.toml"},
        ]});
        let (dir, version) = find_crate(&metadata, "mylib", None).unwrap();
        assert_eq!(dir, PathBuf::from("/deps/core"));
        assert_eq!(version, "0.3.0");
        assert!(find_crate(&metadata, "missing", None).is_err());
        assert!(find_crate(&metadata, "mylib", Some("0.2.0")).is_err());
    }

    #[test]
    fn find_crate_refuses_to_guess_between_versions() {
        let metadata = serde_json::json!({"packages": [
            {"name": "mylib", "version": "0.3.0", "manifest_path": "/a/Cargo.toml"},
            {"name": "mylib", "version": "0.4.0", "manifest_path": "/b/Cargo.toml"},
        ]});
        assert!(find_crate(&metadata, "mylib", None).is_err());
        let (dir, _) = find_crate(&metadata, "mylib", Some("0.4.0")).unwrap();
        assert_eq!(dir, PathBuf::from("/b"));
    }

    #[test]
    fn reachable_crates_follow_normal_dependencies_of_the_root() {
        let package = |id: &str, dir: &str| {
            serde_json::json!({"id": id, "name": id, "version": "1.0.0",
                "manifest_path": format!("{dir}/Cargo.toml")})
        };
        let dep = |pkg: &str, kind: serde_json::Value| serde_json::json!({"pkg": pkg, "dep_kinds": [{"kind": kind}]});
        let metadata = serde_json::json!({
            "packages": [
                package("app", "/app"),
                package("tragwerk", "/t"),
                package("mylib", "/t/mylib"),
                package("devtool", "/devtool"),
                package("sibling", "/sibling"),
            ],
            "resolve": {"nodes": [
                {"id": "app", "deps": [
                    dep("tragwerk", serde_json::Value::Null),
                    dep("devtool", "dev".into()),
                ]},
                {"id": "tragwerk", "deps": [dep("mylib", serde_json::Value::Null)]},
                {"id": "mylib", "deps": []},
                {"id": "sibling", "deps": [dep("mylib", serde_json::Value::Null)]},
            ]},
        });
        let mut names: Vec<String> = reachable_crates(&metadata, Path::new("/app"))
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        names.sort();
        assert_eq!(names, ["mylib", "tragwerk"]);
        assert!(reachable_crates(&metadata, Path::new("/elsewhere")).is_empty());
    }

    /// A crate and a project in a fresh temporary directory.
    struct Fixture {
        root: PathBuf,
    }

    const LIB: &str = "tragwerk";

    impl Fixture {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "tusker-vendor-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            for dir in ["lib/db/migrations", "app/db/migrations"] {
                fs::create_dir_all(root.join(dir)).unwrap();
            }
            fs::write(root.join("lib/tusker.toml"), "").unwrap();
            let fixture = Self { root };
            fixture.write(LIB, "0001_init.sql", "CREATE DOMAIN d AS int;\n");
            fixture.write(LIB, "0002_auth_init.sql", "CREATE TABLE auth_user ();\n");
            fixture.write(
                LIB,
                "0003_auth_add_name.sql",
                "ALTER TABLE auth_user ADD name text;\n",
            );
            fixture
        }

        fn dir(&self, krate: &str) -> PathBuf {
            let short = if krate == LIB { "lib" } else { krate };
            self.root.join(short).join("db/migrations")
        }

        fn write(&self, krate: &str, file: &str, sql: &str) {
            fs::write(self.dir(krate).join(file), sql).unwrap();
        }

        fn read(&self, krate: &str, file: &str) -> String {
            fs::read_to_string(self.dir(krate).join(file)).unwrap()
        }

        fn dependency(&self) -> Result<Dependency> {
            Dependency::load_dir(LIB, &self.root.join("lib"), Some("0.1.0".into()))
        }

        fn project(&self) -> Vec<ProjectMigration> {
            let pattern = self.dir("app").join("*.sql");
            load_project_migrations(&pattern.to_string_lossy()).unwrap()
        }

        fn analyze(&self) -> Report {
            analyze(&[self.dependency().unwrap()], &self.project())
        }

        /// Analyzes, then writes what is pending.
        fn vendor(&self) -> Vec<String> {
            let report = self.analyze();
            assert!(report.problems.is_empty(), "{:?}", report.problems);
            write_pending(&report, &self.project(), &self.dir("app"))
                .unwrap()
                .iter()
                .map(|p| file_name(p))
                .collect()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn pending_names(report: &Report) -> Vec<String> {
        report
            .pending
            .iter()
            .map(|p| format!("{} {}", p.dependency, p.migration.file_name))
            .collect()
    }

    #[test]
    fn vendors_after_the_projects_migrations() {
        let fixture = Fixture::new("append");
        fixture.write("app", "0001_app_init.sql", "CREATE TABLE thing ();\n");
        let report = fixture.analyze();
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert_eq!(
            pending_names(&report),
            [
                "tragwerk 0001_init.sql",
                "tragwerk 0002_auth_init.sql",
                "tragwerk 0003_auth_add_name.sql",
            ]
        );
        assert_eq!(
            fixture.vendor(),
            [
                "0002_tragwerk_init.sql",
                "0003_tragwerk_auth_init.sql",
                "0004_tragwerk_auth_add_name.sql"
            ]
        );
        let copy = fixture.read("app", "0003_tragwerk_auth_init.sql");
        assert!(copy.contains("--   crate: tragwerk 0.1.0\n--   source: 0002_auth_init.sql\n"));
        assert!(fixture.analyze().is_clean());
    }

    #[test]
    fn a_new_release_is_pending() {
        let fixture = Fixture::new("release");
        let _ = fixture.vendor();
        fixture.write(
            LIB,
            "0004_auth_add_email.sql",
            "ALTER TABLE auth_user ADD email text;\n",
        );
        let report = fixture.analyze();
        assert!(report.problems.is_empty());
        assert_eq!(pending_names(&report), ["tragwerk 0004_auth_add_email.sql"]);
    }

    #[test]
    fn an_edited_copy_is_reported() {
        let fixture = Fixture::new("edited");
        let _ = fixture.vendor();
        let copy = fixture.read("app", "0001_tragwerk_init.sql");
        fixture.write(
            "app",
            "0001_tragwerk_init.sql",
            &copy.replace("int", "bigint"),
        );
        assert_eq!(
            fixture.analyze().problems,
            [Problem::EditedCopy {
                file: "0001_tragwerk_init.sql".into()
            }]
        );
    }

    #[test]
    fn an_edited_dependency_migration_is_reported() {
        let fixture = Fixture::new("upstream");
        let _ = fixture.vendor();
        fixture.write(LIB, "0001_init.sql", "CREATE DOMAIN d AS bigint;\n");
        assert_eq!(
            fixture.analyze().problems,
            [Problem::ChangedUpstream {
                file: "0001_tragwerk_init.sql".into(),
                source: "0001_init.sql".into()
            }]
        );
    }

    #[test]
    fn a_copy_before_an_earlier_one_is_reported() {
        let fixture = Fixture::new("order");
        let _ = fixture.vendor();
        let init = fixture.read("app", "0001_tragwerk_init.sql");
        let auth = fixture.read("app", "0002_tragwerk_auth_init.sql");
        fixture.write("app", "0001_tragwerk_init.sql", &auth);
        fixture.write("app", "0002_tragwerk_auth_init.sql", &init);
        assert_eq!(
            fixture.analyze().problems,
            [Problem::Order {
                file: "0001_tragwerk_init.sql".into(),
                missing: "tragwerk 0001_init.sql".into()
            }]
        );
    }

    #[test]
    fn a_copy_from_an_unlisted_crate_is_reported() {
        let fixture = Fixture::new("unlisted");
        let header = Header {
            dependency: "gone".into(),
            version: None,
            source: "0001_init.sql".into(),
            sha256: sha256_hex(b"SELECT 1;\n"),
        };
        fixture.write(
            "app",
            "0001_x.sql",
            &render_vendored(&header, "SELECT 1;\n"),
        );
        assert!(fixture.analyze().problems.iter().any(
            |p| matches!(p, Problem::Unknown { reason, .. } if reason.contains("no [vendor.gone]"))
        ));
    }

    #[test]
    fn a_crate_with_vendored_migrations_is_refused() {
        let fixture = Fixture::new("nested");
        let header = Header {
            dependency: "other".into(),
            version: None,
            source: "0001_init.sql".into(),
            sha256: sha256_hex(b"SELECT 1;\n"),
        };
        fixture.write(
            LIB,
            "0004_other_init.sql",
            &render_vendored(&header, "SELECT 1;\n"),
        );
        let error = fixture.dependency().unwrap_err().to_string();
        assert!(error.contains("vendored from other"), "{error}");
    }

    #[test]
    fn ships_migrations_needs_a_tusker_toml_and_migrations() {
        let fixture = Fixture::new("ships");
        assert!(ships_migrations(&fixture.root.join("lib")));
        assert!(!ships_migrations(&fixture.root.join("app")));
        for file in [
            "0001_init.sql",
            "0002_auth_init.sql",
            "0003_auth_add_name.sql",
        ] {
            fs::remove_file(fixture.dir(LIB).join(file)).unwrap();
        }
        assert!(!ships_migrations(&fixture.root.join("lib")));
    }
}
