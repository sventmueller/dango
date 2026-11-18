//! The Containers panel's model (E136), without AppKit: Compose projects with their services, containers of no
//! project, images and volumes, as rows of a tree. A Compose file of the folder shows as a project even while none of
//! its containers exists.

use std::path::{Path, PathBuf};

use crate::{Container, Image, Volume};

/// How a row stands, which the panel shows as a shape and a word, not as a colour alone (E101).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// Runs.
    Running,
    /// Some of its containers run.
    Partly,
    /// Stopped, exited or created.
    Stopped,
    /// Neither runs nor stops: a group, an image or a volume.
    Plain,
}

/// What a row stands for.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Node {
    /// A section's title: Compose, Containers, Images, Volumes.
    Section(Section),
    /// A Compose project by name, with its file in the folder if it has one.
    Project(String, Option<PathBuf>),
    /// A container by id.
    Container(String),
    /// An image by its reference.
    Image(String),
    /// A volume by name.
    Volume(String),
}

/// The sections of the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    /// Compose projects.
    Compose,
    /// Containers of no project.
    Containers,
    /// Images.
    Images,
    /// Volumes.
    Volumes,
}

/// One row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// How deep it is.
    pub level: usize,
    /// What it says.
    pub label: String,
    /// What it says beside, such as `Up 2 hours` or a size.
    pub detail: String,
    /// How it stands.
    pub look: Look,
    /// What it stands for.
    pub node: Node,
}

/// What Docker shows for one context.
#[derive(Debug, Clone, Default)]
#[expect(missing_docs, reason = "the names say it")]
pub struct Board {
    pub containers: Vec<Container>,
    pub images: Vec<Image>,
    pub volumes: Vec<Volume>,
    /// The Compose files of the window's folder.
    pub files: Vec<PathBuf>,
}

impl Board {
    /// The container with `id`.
    pub fn container(&self, id: &str) -> Option<&Container> {
        self.containers.iter().find(|c| c.id == id)
    }

    /// The Compose file of project `name` in the folder, if it has one.
    pub(crate) fn file_of(&self, name: &str) -> Option<&Path> {
        self.files
            .iter()
            .find(|f| crate::project_of(f) == name)
            .map(PathBuf::as_path)
    }

    /// The rows of the tree, sections without anything left out.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut projects: Vec<String> = self.files.iter().map(|f| crate::project_of(f)).collect();
        for container in &self.containers {
            if let Some(project) = &container.project
                && !projects.contains(project)
            {
                projects.push(project.clone());
            }
        }
        if !projects.is_empty() {
            rows.push(section(Section::Compose));
            for project in projects {
                let members: Vec<&Container> = self
                    .containers
                    .iter()
                    .filter(|c| c.project.as_deref() == Some(project.as_str()))
                    .collect();
                let running = members.iter().filter(|c| c.running()).count();
                let look = match (running, members.len()) {
                    (0, _) => Look::Stopped,
                    (r, n) if r == n => Look::Running,
                    _ => Look::Partly,
                };
                rows.push(Row {
                    level: 1,
                    label: project.clone(),
                    detail: format!("{running}/{}", members.len()),
                    look,
                    node: Node::Project(project.clone(), self.file_of(&project).map(Path::to_path_buf)),
                });
                for container in members {
                    rows.push(container_row(container, 2, container.service.clone()));
                }
            }
        }
        let alone: Vec<&Container> = self.containers.iter().filter(|c| c.project.is_none()).collect();
        if !alone.is_empty() {
            rows.push(section(Section::Containers));
            rows.extend(alone.into_iter().map(|c| container_row(c, 1, None)));
        }
        if !self.images.is_empty() {
            rows.push(section(Section::Images));
            rows.extend(self.images.iter().map(|image| Row {
                level: 1,
                label: image.reference(),
                detail: image.size.clone(),
                look: Look::Plain,
                node: Node::Image(image.reference()),
            }));
        }
        if !self.volumes.is_empty() {
            rows.push(section(Section::Volumes));
            rows.extend(self.volumes.iter().map(|volume| Row {
                level: 1,
                label: volume.name.clone(),
                detail: volume.driver.clone(),
                look: Look::Plain,
                node: Node::Volume(volume.name.clone()),
            }));
        }
        rows
    }
}

fn section(which: Section) -> Row {
    Row {
        level: 0,
        label: String::new(),
        detail: String::new(),
        look: Look::Plain,
        node: Node::Section(which),
    }
}

fn container_row(container: &Container, level: usize, service: Option<String>) -> Row {
    Row {
        level,
        label: service.unwrap_or_else(|| container.name.clone()),
        detail: container.status.clone(),
        look: if container.running() {
            Look::Running
        } else {
            Look::Stopped
        },
        node: Node::Container(container.id.clone()),
    }
}
