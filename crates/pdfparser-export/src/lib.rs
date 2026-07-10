//! Export helpers.
#![deny(missing_docs)]

use pdfparser_ir::ExtractedDocument;

/// Serialize extract to pretty JSON.
pub fn to_json_pretty(doc: &ExtractedDocument) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(doc)
}

/// Serialize extract to compact JSON.
pub fn to_json(doc: &ExtractedDocument) -> Result<String, serde_json::Error> {
    serde_json::to_string(doc)
}
