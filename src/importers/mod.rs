mod postman;

use anyhow::{anyhow, Context, Result};
use std::path::Path;

use crate::entities::RequestData;

pub use postman::PostmanCollectionImporter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportWarning {
    pub path: Option<String>,
    pub message: String,
}

impl ImportWarning {
    pub fn new(path: Option<String>, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ImportedNode {
    Folder {
        name: String,
        children: Vec<ImportedNode>,
    },
    Request {
        request: RequestData,
    },
}

impl ImportedNode {
    pub fn folder_count(&self) -> usize {
        match self {
            ImportedNode::Folder { children, .. } => {
                1 + children
                    .iter()
                    .map(ImportedNode::folder_count)
                    .sum::<usize>()
            }
            ImportedNode::Request { .. } => 0,
        }
    }

    pub fn request_count(&self) -> usize {
        match self {
            ImportedNode::Folder { children, .. } => {
                children.iter().map(ImportedNode::request_count).sum()
            }
            ImportedNode::Request { .. } => 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImportedCollection {
    pub name: String,
    pub nodes: Vec<ImportedNode>,
}

impl ImportedCollection {
    pub fn folder_count(&self) -> usize {
        self.nodes.iter().map(ImportedNode::folder_count).sum()
    }

    pub fn request_count(&self) -> usize {
        self.nodes.iter().map(ImportedNode::request_count).sum()
    }
}

#[derive(Debug, Clone)]
pub struct ImportResult {
    pub provider: &'static str,
    pub collection: ImportedCollection,
    pub warnings: Vec<ImportWarning>,
}

pub trait CollectionImporter {
    fn provider_name(&self) -> &'static str;
    fn matches(&self, path: &Path, contents: &str) -> bool;
    fn import(&self, path: &Path, contents: &str) -> Result<ImportResult>;
}

pub struct ImportRegistry {
    importers: Vec<Box<dyn CollectionImporter>>,
}

impl Default for ImportRegistry {
    fn default() -> Self {
        Self {
            importers: vec![Box::new(PostmanCollectionImporter::default())],
        }
    }
}

impl ImportRegistry {
    pub fn import_file(&self, path: &Path) -> Result<ImportResult> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        for importer in &self.importers {
            if importer.matches(path, &contents) {
                return importer.import(path, &contents);
            }
        }

        Err(anyhow!(
            "Unsupported collection file. Only Postman collection JSON is supported right now."
        ))
    }
}
