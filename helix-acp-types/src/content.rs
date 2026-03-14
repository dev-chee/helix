//! Content block types for ACP messages

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A content block that can appear in messages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ContentBlock {
    /// Text content
    Text(TextContent),
    /// Image content
    Image(ImageContent),
    /// Audio content
    Audio(AudioContent),
    /// Resource link
    ResourceLink(ResourceLink),
    /// Embedded resource
    Resource(ResourceContent),
}

impl From<String> for ContentBlock {
    fn from(text: String) -> Self {
        ContentBlock::Text(TextContent { text })
    }
}

impl From<&str> for ContentBlock {
    fn from(text: &str) -> Self {
        ContentBlock::Text(TextContent {
            text: text.to_string(),
        })
    }
}

/// Text content block
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextContent {
    /// The text content
    pub text: String,
}

/// Image content block
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageContent {
    /// Image data (base64 encoded if from data URI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// MIME type of the image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// URL to the image if remote
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Alt text for accessibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
}

/// Audio content block
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioContent {
    /// Audio data (base64 encoded if from data URI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// MIME type of the audio
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// URL to the audio if remote
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Resource link - a reference to an external resource
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLink {
    /// URI of the resource
    pub uri: String,
    /// Human-readable title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// MIME type if known
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Embedded resource content
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceContent {
    /// URI identifying the resource
    pub uri: String,
    /// MIME type of the resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Text content if the resource is text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Binary content (base64 encoded) if the resource is binary
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

/// File location reference
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileLocation {
    /// Path to the file
    pub path: PathBuf,
    /// Line number (1-based)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Column number (1-based)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// End line for a range
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
    /// End column for a range
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_column: Option<u32>,
}

/// Code block with language information
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeBlock {
    /// The code content
    pub code: String,
    /// Language identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// File path if this code is from a file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<PathBuf>,
}
