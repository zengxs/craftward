// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::str;

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

const APPLICATION_RESOURCE_NAME: &str = "application.txt";
const CATALOG_RESOURCE_NAME: &str = "components.json";
const QRC_FILE_NAME: &str = "legal.qrc";

pub struct ProjectPaths {
    pub manifest: PathBuf,
    pub cache_dir: PathBuf,
    pub output_dir: PathBuf,
}

impl Default for ProjectPaths {
    fn default() -> Self {
        let xtask_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repository_dir = xtask_dir
            .parent()
            .expect("xtask must be located directly under the repository root");

        Self {
            manifest: repository_dir.join("licenses.toml"),
            cache_dir: xtask_dir.join("data/licenses"),
            output_dir: repository_dir.join("app/build/generated/legal"),
        }
    }
}

pub fn fetch(manifest_path: &Path, cache_dir: &Path) -> Result<usize> {
    fetch_with(manifest_path, cache_dir, download)
}

pub fn generate(manifest_path: &Path, cache_dir: &Path, output_dir: &Path) -> Result<usize> {
    let project = Project::load(manifest_path, cache_dir)?;
    let generated = project.render()?;
    generated.write_to(output_dir)?;
    Ok(generated.document_count)
}

pub fn check(manifest_path: &Path, cache_dir: &Path) -> Result<usize> {
    let project = Project::load(manifest_path, cache_dir)?;
    let generated = project.render()?;
    Ok(generated.document_count)
}

fn fetch_with<F>(manifest_path: &Path, cache_dir: &Path, mut downloader: F) -> Result<usize>
where
    F: FnMut(&str) -> Result<Vec<u8>>,
{
    let project = Project::load(manifest_path, cache_dir)?;
    fs::create_dir_all(cache_dir).with_context(|| {
        format!(
            "failed to create the license cache directory {}",
            cache_dir.display()
        )
    })?;

    let mut fetched = 0;
    for component in &project.components {
        let LicenseSource::Cached { path, url } = &component.source else {
            continue;
        };

        let bytes = downloader(url)
            .with_context(|| format!("failed to fetch the license for {}", component.name))?;
        str::from_utf8(&bytes)
            .with_context(|| format!("the license for {} is not UTF-8", component.name))?;
        write_if_changed(path, &bytes)?;
        fetched += 1;
    }

    Ok(fetched)
}

fn download(url: &str) -> Result<Vec<u8>> {
    let response = ureq::get(url)
        .call()
        .with_context(|| format!("request to {url} failed"))?;
    let mut body = response.into_body();
    body.read_to_vec()
        .with_context(|| format!("failed to read the response from {url}"))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct LicenseManifest {
    spdx_license_list_version: String,
    application: ApplicationLicense,
    #[serde(default)]
    component: Vec<ComponentEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct ApplicationLicense {
    license_file: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct ComponentEntry {
    name: String,
    spdx_identifier: String,
    website: String,
    license_file: Option<PathBuf>,
    license_url: Option<String>,
    notice_text: Option<String>,
}

struct Project {
    application_license: ManifestFile,
    components: Vec<Component>,
}

struct Component {
    name: String,
    slug: String,
    spdx_identifier: String,
    website: String,
    notice_text: Option<String>,
    source: LicenseSource,
}

enum LicenseSource {
    Local(ManifestFile),
    Cached { path: PathBuf, url: String },
}

struct ManifestFile {
    configured_path: PathBuf,
    resolved_path: PathBuf,
}

impl ManifestFile {
    fn new(manifest_dir: &Path, configured_path: PathBuf) -> Self {
        let resolved_path = manifest_dir.join(&configured_path);
        Self {
            configured_path,
            resolved_path,
        }
    }
}

impl Project {
    fn load(manifest_path: &Path, cache_dir: &Path) -> Result<Self> {
        let manifest_path = manifest_path.canonicalize().with_context(|| {
            format!(
                "failed to locate the license manifest {}",
                manifest_path.display()
            )
        })?;
        let manifest_dir = manifest_path
            .parent()
            .expect("a canonical manifest path must have a parent");
        let source = fs::read_to_string(&manifest_path).with_context(|| {
            format!(
                "failed to read the license manifest {}",
                manifest_path.display()
            )
        })?;
        let manifest: LicenseManifest = toml::from_str(&source).with_context(|| {
            format!(
                "failed to parse the license manifest {}",
                manifest_path.display()
            )
        })?;

        ensure!(
            !manifest.spdx_license_list_version.trim().is_empty(),
            "spdx-license-list-version must not be empty"
        );
        ensure_relative_path(
            &manifest.application.license_file,
            "application.license-file",
        )?;

        let mut names = HashSet::new();
        let mut slugs = HashSet::new();
        let mut components = Vec::with_capacity(manifest.component.len());

        for entry in manifest.component {
            let name = entry.name.trim();
            ensure!(!name.is_empty(), "component name must not be empty");
            ensure!(
                names.insert(name.to_lowercase()),
                "component name {name:?} is duplicated"
            );

            let slug = slugify(name)?;
            ensure!(
                slugs.insert(slug.clone()),
                "component name {name:?} has the duplicate resource name {slug:?}"
            );

            let spdx_identifier = entry.spdx_identifier.trim();
            ensure!(
                is_spdx_license_id(spdx_identifier),
                "component {name:?} has an invalid SPDX license identifier"
            );
            ensure!(
                !entry.website.trim().is_empty(),
                "component {name:?} website must not be empty"
            );

            let source = match (entry.license_file, entry.license_url) {
                (Some(_), Some(_)) => {
                    bail!("component {name:?} must not define both license-file and license-url")
                }
                (Some(path), None) => {
                    ensure_relative_path(&path, &format!("component {name:?} license-file"))?;
                    LicenseSource::Local(ManifestFile::new(manifest_dir, path))
                }
                (None, explicit_url) => {
                    let url = explicit_url.unwrap_or_else(|| {
                        format!(
                            "https://raw.githubusercontent.com/spdx/license-list-data/v{}/text/{spdx_identifier}.txt",
                            manifest.spdx_license_list_version
                        )
                    });
                    ensure!(
                        url.starts_with("https://"),
                        "component {name:?} license URL must use HTTPS"
                    );
                    LicenseSource::Cached {
                        path: cache_dir.join(format!("{slug}.txt")),
                        url,
                    }
                }
            };

            components.push(Component {
                name: name.to_owned(),
                slug,
                spdx_identifier: spdx_identifier.to_owned(),
                website: entry.website,
                notice_text: entry.notice_text,
                source,
            });
        }

        components.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.name.cmp(&right.name))
        });

        Ok(Self {
            application_license: ManifestFile::new(manifest_dir, manifest.application.license_file),
            components,
        })
    }

    fn render(&self) -> Result<GeneratedResources> {
        let application_text = read_manifest_license(&self.application_license, "application")?;
        let mut documents = vec![GeneratedDocument {
            resource_name: APPLICATION_RESOURCE_NAME.to_owned(),
            contents: application_text,
        }];
        let mut catalog = Vec::with_capacity(self.components.len());

        for component in &self.components {
            let original = match &component.source {
                LicenseSource::Local(file) => read_manifest_license(file, &component.name)?,
                LicenseSource::Cached { path, .. } => read_license(path, &component.name)
                    .with_context(|| {
                        format!(
                            "failed to load the cached license for {}; run `xtask licenses fetch`",
                            component.name
                        )
                    })?,
            };
            let resource_name = format!("{}.txt", component.slug);
            let contents = compose_document(component.notice_text.as_deref(), original);

            catalog.push(CatalogComponent {
                component: component.name.clone(),
                spdx_identifier: component.spdx_identifier.clone(),
                legal_text: format!("qrc:///legal/{resource_name}"),
                website: component.website.clone(),
            });
            documents.push(GeneratedDocument {
                resource_name,
                contents,
            });
        }

        let catalog = serde_json::to_vec_pretty(&Catalog {
            components: catalog,
        })
        .context("failed to serialize the legal catalog")?;
        let qrc = render_qrc(&documents);
        let document_count = documents.len();

        Ok(GeneratedResources {
            catalog,
            documents,
            qrc,
            document_count,
        })
    }
}

struct GeneratedResources {
    catalog: Vec<u8>,
    documents: Vec<GeneratedDocument>,
    qrc: Vec<u8>,
    document_count: usize,
}

struct GeneratedDocument {
    resource_name: String,
    contents: Vec<u8>,
}

impl GeneratedResources {
    fn write_to(&self, output_dir: &Path) -> Result<()> {
        let texts_dir = output_dir.join("texts");
        fs::create_dir_all(&texts_dir).with_context(|| {
            format!(
                "failed to create the generated legal resource directory {}",
                texts_dir.display()
            )
        })?;

        let mut catalog = self.catalog.clone();
        catalog.push(b'\n');
        write_if_changed(&output_dir.join(CATALOG_RESOURCE_NAME), &catalog)?;
        write_if_changed(&output_dir.join(QRC_FILE_NAME), &self.qrc)?;

        for document in &self.documents {
            write_if_changed(&texts_dir.join(&document.resource_name), &document.contents)?;
        }

        Ok(())
    }
}

#[derive(Serialize)]
struct Catalog {
    components: Vec<CatalogComponent>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogComponent {
    component: String,
    spdx_identifier: String,
    legal_text: String,
    website: String,
}

fn ensure_relative_path(path: &Path, field: &str) -> Result<()> {
    ensure!(!path.as_os_str().is_empty(), "{field} must not be empty");
    ensure!(
        path.is_relative(),
        "{field} must be relative to licenses.toml"
    );
    Ok(())
}

fn is_spdx_license_id(identifier: &str) -> bool {
    !identifier.is_empty()
        && identifier
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'+'))
}

fn slugify(name: &str) -> Result<String> {
    let mut slug = String::new();
    let mut needs_separator = false;

    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            if needs_separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character.to_ascii_lowercase());
            needs_separator = false;
        } else {
            needs_separator = true;
        }
    }

    ensure!(
        !slug.is_empty(),
        "component name {name:?} cannot be converted to a resource name"
    );
    Ok(slug)
}

fn read_license(path: &Path, label: &str) -> Result<Vec<u8>> {
    let contents = fs::read(path)
        .with_context(|| format!("failed to read the {label} license at {}", path.display()))?;
    str::from_utf8(&contents)
        .with_context(|| format!("the {label} license at {} is not UTF-8", path.display()))?;
    Ok(contents)
}

fn read_manifest_license(file: &ManifestFile, label: &str) -> Result<Vec<u8>> {
    read_license(&file.resolved_path, label).with_context(|| {
        format!(
            "failed to load the {label} license from {} (resolved to {})",
            file.configured_path.display(),
            file.resolved_path.display()
        )
    })
}

fn compose_document(notice_text: Option<&str>, original: Vec<u8>) -> Vec<u8> {
    let Some(notice_text) = notice_text.map(str::trim).filter(|text| !text.is_empty()) else {
        return original;
    };

    let mut contents =
        format!("CRAFTWARD NOTICE\n================\n\n{notice_text}\n\nLICENSE\n=======\n\n")
            .into_bytes();
    contents.extend_from_slice(&original);
    contents
}

fn render_qrc(documents: &[GeneratedDocument]) -> Vec<u8> {
    let mut qrc = String::from("<RCC>\n  <qresource prefix=\"/legal\">\n");
    qrc.push_str("    <file alias=\"components.json\">components.json</file>\n");
    for document in documents {
        qrc.push_str(&format!(
            "    <file alias=\"{}\">texts/{}</file>\n",
            document.resource_name, document.resource_name
        ));
    }
    qrc.push_str("  </qresource>\n</RCC>\n");
    qrc.into_bytes()
}

fn write_if_changed(path: &Path, contents: &[u8]) -> Result<()> {
    if fs::read(path).is_ok_and(|existing| existing == contents) {
        return Ok(());
    }

    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static NEXT_TEST_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(name: &str) -> Self {
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target/test-work")
                .join(format!("{name}-{}-{sequence}", std::process::id()));
            if path.exists() {
                fs::remove_dir_all(&path).unwrap();
            }
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn generates_catalog_documents_and_qrc_from_manifest_relative_sources() {
        let test_dir = TestDirectory::new("generate");
        let manifest_path = test_dir.path().join("licenses.toml");
        let cache_dir = test_dir.path().join("cache");
        let output_dir = test_dir.path().join("generated");

        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(test_dir.path().join("COPYING.md"), "Application license\n").unwrap();
        fs::write(test_dir.path().join("local.txt"), "Local license\n").unwrap();
        fs::write(cache_dir.join("remote-library.txt"), "Remote license\n").unwrap();
        fs::write(
            &manifest_path,
            r#"
spdx-license-list-version = "3.28.0"

[application]
license-file = "COPYING.md"

[[component]]
name = "Remote Library"
spdx-identifier = "MIT"
license-url = "https://example.invalid/license"
website = "https://example.invalid/remote"
notice-text = "Used by Craftward."

[[component]]
name = "A Local Library"
spdx-identifier = "HPND"
license-file = "local.txt"
website = "https://example.invalid/local"
"#,
        )
        .unwrap();

        assert_eq!(
            generate(&manifest_path, &cache_dir, &output_dir).unwrap(),
            3
        );
        assert_eq!(
            fs::read_to_string(output_dir.join("texts/application.txt")).unwrap(),
            "Application license\n"
        );
        assert_eq!(
            fs::read_to_string(output_dir.join("texts/a-local-library.txt")).unwrap(),
            "Local license\n"
        );
        assert_eq!(
            fs::read_to_string(output_dir.join("texts/remote-library.txt")).unwrap(),
            "CRAFTWARD NOTICE\n================\n\nUsed by Craftward.\n\nLICENSE\n=======\n\nRemote license\n"
        );

        let catalog = fs::read_to_string(output_dir.join("components.json")).unwrap();
        assert!(catalog.find("A Local Library").unwrap() < catalog.find("Remote Library").unwrap());
        assert!(catalog.contains("qrc:///legal/remote-library.txt"));

        let qrc = fs::read_to_string(output_dir.join("legal.qrc")).unwrap();
        assert!(qrc.contains("alias=\"application.txt\""));
        assert!(qrc.contains("texts/remote-library.txt"));
    }

    #[test]
    fn fetches_remote_and_spdx_sources_but_not_local_sources() {
        let test_dir = TestDirectory::new("fetch");
        let manifest_path = test_dir.path().join("licenses.toml");
        let cache_dir = test_dir.path().join("cache");
        fs::write(test_dir.path().join("COPYING.md"), "Application license\n").unwrap();
        fs::write(test_dir.path().join("local.txt"), "Local license\n").unwrap();
        fs::write(
            &manifest_path,
            r#"
spdx-license-list-version = "3.28.0"

[application]
license-file = "COPYING.md"

[[component]]
name = "SPDX Source"
spdx-identifier = "Apache-2.0"
website = "https://example.invalid/spdx"

[[component]]
name = "Explicit Source"
spdx-identifier = "MIT"
license-url = "https://example.invalid/license"
website = "https://example.invalid/explicit"

[[component]]
name = "Local Source"
spdx-identifier = "HPND"
license-file = "local.txt"
website = "https://example.invalid/local"
"#,
        )
        .unwrap();

        let mut urls = Vec::new();
        let count = fetch_with(&manifest_path, &cache_dir, |url| {
            urls.push(url.to_owned());
            Ok(format!("License from {url}\n").into_bytes())
        })
        .unwrap();

        assert_eq!(count, 2);
        assert!(urls.contains(&"https://example.invalid/license".to_owned()));
        assert!(urls.iter().any(|url| {
            url == "https://raw.githubusercontent.com/spdx/license-list-data/v3.28.0/text/Apache-2.0.txt"
        }));
        assert!(cache_dir.join("explicit-source.txt").is_file());
        assert!(cache_dir.join("spdx-source.txt").is_file());
        assert!(!cache_dir.join("local-source.txt").exists());
    }

    #[test]
    fn rejects_absolute_license_file_paths() {
        let test_dir = TestDirectory::new("absolute-path");
        let manifest_path = test_dir.path().join("licenses.toml");
        fs::write(
            &manifest_path,
            format!(
                r#"
spdx-license-list-version = "3.28.0"

[application]
license-file = {:?}
"#,
                test_dir.path().join("COPYING.md").display().to_string()
            ),
        )
        .unwrap();

        let error = check(&manifest_path, &test_dir.path().join("cache")).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("application.license-file must be relative")
        );
    }

    #[test]
    fn rejects_ambiguous_component_sources() {
        let test_dir = TestDirectory::new("ambiguous-source");
        let manifest_path = test_dir.path().join("licenses.toml");
        fs::write(test_dir.path().join("COPYING.md"), "Application license\n").unwrap();
        fs::write(
            &manifest_path,
            r#"
spdx-license-list-version = "3.28.0"

[application]
license-file = "COPYING.md"

[[component]]
name = "Ambiguous"
spdx-identifier = "MIT"
license-file = "local.txt"
license-url = "https://example.invalid/license"
website = "https://example.invalid"
"#,
        )
        .unwrap();

        let error = check(&manifest_path, &test_dir.path().join("cache")).unwrap_err();
        assert!(error.to_string().contains("must not define both"));
    }

    #[test]
    fn reports_manifest_relative_and_resolved_local_paths() {
        let test_dir = TestDirectory::new("missing-local-source");
        let manifest_path = test_dir.path().join("licenses.toml");
        fs::write(test_dir.path().join("COPYING.md"), "Application license\n").unwrap();
        fs::write(
            &manifest_path,
            r#"
spdx-license-list-version = "3.28.0"

[application]
license-file = "COPYING.md"

[[component]]
name = "Missing Source"
spdx-identifier = "HPND"
license-file = "licenses/missing.txt"
website = "https://example.invalid"
"#,
        )
        .unwrap();

        let error = check(&manifest_path, &test_dir.path().join("cache")).unwrap_err();
        let error = format!("{error:#}");
        assert!(error.contains("licenses/missing.txt"));
        assert!(error.contains(&test_dir.path().display().to_string()));
    }
}
