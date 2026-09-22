use super::content::get_content_items;
use super::get::get_many;
use crate::state::{CacheBehaviour, ContentItem, ModLoader, ProjectType};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct ComparisonReport {
    pub instances: Vec<ComparedInstance>,
    pub content: Vec<ContentComparisonRow>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComparedInstance {
    pub instance_id: String,
    pub name: String,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentComparisonEntry {
    pub file_name: String,
    pub version_number: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonState {
    Identical,
    OnlyInSome,
    VersionDiffers,
    EnabledDiffers,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContentComparisonRow {
    pub project_type: ProjectType,
    pub display_name: String,
    pub entries: Vec<Option<ContentComparisonEntry>>,
    /// Every state that applies to this row. A row can be both `OnlyInSome`
    /// and `VersionDiffers`/`EnabledDiffers` at once (e.g. present in two of
    /// three instances, with those two on different versions). `Identical`
    /// only ever appears alone.
    pub states: Vec<ComparisonState>,
}

#[tracing::instrument]
pub async fn compare_instances(
    instance_ids: Vec<String>,
) -> crate::Result<ComparisonReport> {
    if instance_ids.len() < 2 {
        return Err(crate::ErrorKind::InputError(
            "At least two instances are required to compare".to_string(),
        )
        .into());
    }

    let id_refs =
        instance_ids.iter().map(String::as_str).collect::<Vec<_>>();
    let metadata_by_id = get_many(&id_refs)
        .await?
        .into_iter()
        .map(|metadata| (metadata.instance.id.clone(), metadata))
        .collect::<HashMap<_, _>>();

    let mut instances = Vec::with_capacity(instance_ids.len());
    let mut content_per_instance = Vec::with_capacity(instance_ids.len());
    for instance_id in &instance_ids {
        let metadata = metadata_by_id.get(instance_id).ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Instance {instance_id} not found"
            ))
        })?;

        instances.push(ComparedInstance {
            instance_id: instance_id.clone(),
            name: metadata.instance.name.clone(),
            game_version: metadata.applied_content_set.game_version.clone(),
            loader: metadata.applied_content_set.loader,
            loader_version: metadata
                .applied_content_set
                .loader_version
                .clone(),
        });

        let items = get_content_items(
            instance_id,
            Some(CacheBehaviour::StaleWhileRevalidateSkipOffline),
        )
        .await?;
        content_per_instance.push(items);
    }

    let content = build_content_rows(&content_per_instance);

    Ok(ComparisonReport { instances, content })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ContentMatchKey {
    Project(String),
    FileName(ProjectType, String),
    Hash(String),
}

fn match_key(item: &ContentItem) -> ContentMatchKey {
    if let Some(project) = &item.project {
        ContentMatchKey::Project(project.id.clone())
    } else if !item.file_name.is_empty() {
        ContentMatchKey::FileName(
            item.project_type,
            item.file_name.to_lowercase(),
        )
    } else {
        ContentMatchKey::Hash(item.id.clone())
    }
}

fn build_content_rows(
    content_per_instance: &[Vec<ContentItem>],
) -> Vec<ContentComparisonRow> {
    let instance_count = content_per_instance.len();
    let mut order = Vec::new();
    let mut rows: HashMap<
        ContentMatchKey,
        (ProjectType, Vec<Option<ContentComparisonEntry>>),
    > = HashMap::new();

    for (index, items) in content_per_instance.iter().enumerate() {
        for item in items {
            let key = match_key(item);
            let (_, entries) = rows.entry(key.clone()).or_insert_with(|| {
                order.push(key.clone());
                (item.project_type, vec![None; instance_count])
            });

            entries[index] = Some(ContentComparisonEntry {
                file_name: item.file_name.clone(),
                version_number: item
                    .version
                    .as_ref()
                    .map(|version| version.version_number.clone()),
                enabled: item.enabled,
            });
        }
    }

    let mut display_names = HashMap::new();
    for items in content_per_instance {
        for item in items {
            let key = match_key(item);
            display_names.entry(key).or_insert_with(|| {
                item.project
                    .as_ref()
                    .map(|project| project.title.clone())
                    .unwrap_or_else(|| item.file_name.clone())
            });
        }
    }

    let mut result = order
        .into_iter()
        .filter_map(|key| {
            let (project_type, entries) = rows.remove(&key)?;
            let display_name = display_names
                .remove(&key)
                .unwrap_or_else(|| "Unknown".to_string());
            let states = determine_states(&entries);

            Some(ContentComparisonRow {
                project_type,
                display_name,
                entries,
                states,
            })
        })
        .collect::<Vec<_>>();

    result.sort_by(|left, right| {
        left.project_type
            .get_name()
            .cmp(right.project_type.get_name())
            .then_with(|| {
                left.display_name
                    .to_lowercase()
                    .cmp(&right.display_name.to_lowercase())
            })
    });

    result
}

fn determine_states(
    entries: &[Option<ContentComparisonEntry>],
) -> Vec<ComparisonState> {
    let present = entries.iter().flatten().collect::<Vec<_>>();
    let mut states = Vec::new();

    if present.len() != entries.len() {
        states.push(ComparisonState::OnlyInSome);
    }

    if let Some(first) = present.first() {
        if present
            .iter()
            .any(|entry| entry.version_number != first.version_number)
        {
            states.push(ComparisonState::VersionDiffers);
        }

        if present.iter().any(|entry| entry.enabled != first.enabled) {
            states.push(ComparisonState::EnabledDiffers);
        }
    }

    if states.is_empty() {
        states.push(ComparisonState::Identical);
    }

    states
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonExportFormat {
    Markdown,
    Json,
}

#[tracing::instrument]
pub async fn export_comparison(
    instance_ids: Vec<String>,
    format: ComparisonExportFormat,
) -> crate::Result<String> {
    let report = compare_instances(instance_ids).await?;

    match format {
        ComparisonExportFormat::Markdown => Ok(report.to_markdown()),
        ComparisonExportFormat::Json => report.to_json(),
    }
}

impl ComparisonReport {
    pub fn to_json(&self) -> crate::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Instance Comparison\n\n");

        out.push_str("| | ");
        out.push_str(
            &self
                .instances
                .iter()
                .map(|instance| instance.name.as_str())
                .collect::<Vec<_>>()
                .join(" | "),
        );
        out.push_str(" |\n");
        out.push_str(&format!(
            "|{}|\n",
            "---|".repeat(self.instances.len() + 1)
        ));
        out.push_str("| Minecraft version | ");
        out.push_str(
            &self
                .instances
                .iter()
                .map(|instance| instance.game_version.as_str())
                .collect::<Vec<_>>()
                .join(" | "),
        );
        out.push_str(" |\n");
        out.push_str("| Loader | ");
        out.push_str(
            &self
                .instances
                .iter()
                .map(|instance| {
                    match &instance.loader_version {
                        Some(version) => {
                            format!("{} {}", instance.loader.as_str(), version)
                        }
                        None => instance.loader.as_str().to_string(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" | "),
        );
        out.push_str(" |\n\n");

        for project_type in [
            ProjectType::Mod,
            ProjectType::ResourcePack,
            ProjectType::ShaderPack,
            ProjectType::DataPack,
        ] {
            let rows = self
                .content
                .iter()
                .filter(|row| row.project_type == project_type)
                .collect::<Vec<_>>();
            if rows.is_empty() {
                continue;
            }

            out.push_str(&format!(
                "## {}\n\n",
                project_type_heading(project_type)
            ));
            out.push_str("| | ");
            out.push_str(
                &self
                    .instances
                    .iter()
                    .map(|instance| instance.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" | "),
            );
            out.push_str(" |\n");
            out.push_str(&format!(
                "|{}|\n",
                "---|".repeat(self.instances.len() + 1)
            ));

            for row in rows {
                out.push_str(&format!("| {} | ", row.display_name));
                out.push_str(
                    &row.entries
                        .iter()
                        .map(|entry| format_cell(entry.as_ref()))
                        .collect::<Vec<_>>()
                        .join(" | "),
                );
                out.push_str(" |\n");
            }
            out.push('\n');
        }

        out
    }
}

fn format_cell(entry: Option<&ContentComparisonEntry>) -> String {
    match entry {
        None => "—".to_string(),
        Some(entry) => {
            let label = entry
                .version_number
                .clone()
                .unwrap_or_else(|| entry.file_name.clone());
            if entry.enabled {
                label
            } else {
                format!("{label} (disabled)")
            }
        }
    }
}

fn project_type_heading(project_type: ProjectType) -> &'static str {
    match project_type {
        ProjectType::Mod => "Mods",
        ProjectType::ResourcePack => "Resource Packs",
        ProjectType::ShaderPack => "Shader Packs",
        ProjectType::DataPack => "Data Packs",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ContentItemProject, ContentItemVersion, License};

    fn project(id: &str, title: &str) -> ContentItemProject {
        ContentItemProject {
            id: id.to_string(),
            slug: None,
            title: title.to_string(),
            icon_url: None,
            license: License {
                id: "unknown".to_string(),
                name: "unknown".to_string(),
                url: None,
            },
            categories: Vec::new(),
            additional_categories: Vec::new(),
        }
    }

    fn version(id: &str, version_number: &str) -> ContentItemVersion {
        ContentItemVersion {
            id: id.to_string(),
            version_number: version_number.to_string(),
            file_name: format!("{id}.jar"),
            date_published: None,
        }
    }

    fn item(
        file_name: &str,
        project: Option<ContentItemProject>,
        version: Option<ContentItemVersion>,
        enabled: bool,
    ) -> ContentItem {
        ContentItem {
            file_name: file_name.to_string(),
            file_path: file_name.to_string(),
            id: file_name.to_string(),
            size: 0,
            enabled,
            locked: false,
            project_type: ProjectType::Mod,
            project,
            version,
            environment: None,
            owner: None,
            has_update: false,
            update_version_id: None,
            date_added: None,
            source_kind: None,
            embedded_metadata: None,
        }
    }

    #[test]
    fn identical_when_present_everywhere_with_same_version_and_state() {
        let sodium = |enabled| {
            item(
                "sodium.jar",
                Some(project("sodium", "Sodium")),
                Some(version("v1", "0.5.3")),
                enabled,
            )
        };
        let rows = build_content_rows(&[vec![sodium(true)], vec![sodium(true)]]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].states, vec![ComparisonState::Identical]);
    }

    #[test]
    fn version_differs_when_version_numbers_differ() {
        let a = item(
            "sodium.jar",
            Some(project("sodium", "Sodium")),
            Some(version("v1", "0.5.3")),
            true,
        );
        let b = item(
            "sodium.jar",
            Some(project("sodium", "Sodium")),
            Some(version("v2", "0.5.4")),
            true,
        );
        let rows = build_content_rows(&[vec![a], vec![b]]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].states, vec![ComparisonState::VersionDiffers]);
    }

    #[test]
    fn enabled_differs_when_only_enabled_state_differs() {
        let a = item(
            "sodium.jar",
            Some(project("sodium", "Sodium")),
            Some(version("v1", "0.5.3")),
            true,
        );
        let b = item(
            "sodium.jar",
            Some(project("sodium", "Sodium")),
            Some(version("v1", "0.5.3")),
            false,
        );
        let rows = build_content_rows(&[vec![a], vec![b]]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].states, vec![ComparisonState::EnabledDiffers]);
    }

    #[test]
    fn version_and_enabled_both_differ_produce_both_states() {
        let a = item(
            "wathe.jar",
            Some(project("wathe", "Wathe: Murder Mystery")),
            Some(version("v1", "1.3.2-1.21.1")),
            true,
        );
        let b = item(
            "wathe.jar",
            Some(project("wathe", "Wathe: Murder Mystery")),
            Some(version("v2", "1.4.1-1.21.1")),
            false,
        );
        let rows = build_content_rows(&[vec![a], vec![b]]);

        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].states,
            vec![ComparisonState::VersionDiffers, ComparisonState::EnabledDiffers]
        );
    }

    #[test]
    fn only_in_some_when_missing_from_an_instance() {
        let sodium = item(
            "sodium.jar",
            Some(project("sodium", "Sodium")),
            Some(version("v1", "0.5.3")),
            true,
        );
        let rows = build_content_rows(&[vec![sodium], vec![]]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].states, vec![ComparisonState::OnlyInSome]);
        assert!(rows[0].entries[1].is_none());
    }

    #[test]
    fn only_in_some_also_reports_differences_among_present_entries() {
        let a = item(
            "sodium.jar",
            Some(project("sodium", "Sodium")),
            Some(version("v1", "0.5.3")),
            true,
        );
        let c = item(
            "sodium.jar",
            Some(project("sodium", "Sodium")),
            Some(version("v2", "0.5.4")),
            false,
        );
        let rows = build_content_rows(&[vec![a], vec![], vec![c]]);

        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].states,
            vec![
                ComparisonState::OnlyInSome,
                ComparisonState::VersionDiffers,
                ComparisonState::EnabledDiffers
            ]
        );
    }

    #[test]
    fn matches_by_file_name_when_project_is_unknown() {
        let a = item("local-tweak.jar", None, None, true);
        let b = item("local-tweak.jar", None, None, true);
        let rows = build_content_rows(&[vec![a], vec![b]]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].states, vec![ComparisonState::Identical]);
    }

    #[test]
    fn distinct_projects_produce_distinct_rows() {
        let sodium = item(
            "sodium.jar",
            Some(project("sodium", "Sodium")),
            Some(version("v1", "0.5.3")),
            true,
        );
        let iris = item(
            "iris.jar",
            Some(project("iris", "Iris")),
            Some(version("v1", "1.6.5")),
            true,
        );
        let rows = build_content_rows(&[vec![sodium, iris]]);

        assert_eq!(rows.len(), 2);
    }
}
