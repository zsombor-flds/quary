use quary_proto::ProjectFile;
use std::io::Read;

pub fn deserialize_project_file_from_yaml(read: impl Read) -> Result<ProjectFile, String> {
    serde_yaml::from_reader(read).map_err(|e| format!("reading yaml: {}", e))
}
pub fn serialize_project_file_to_yaml(project_file: ProjectFile) -> Result<String, String> {
    serde_yaml::to_string(&project_file).map_err(|e| format!("writing yaml: {}", e))
}

// Tests names used in the project file
pub const STANDARD_TEST_TYPE_SQL_NOT_NULL: &str = "not_null";
pub const STANDARD_TEST_TYPE_SQL_UNIQUE: &str = "unique";
pub const STANDARD_TEST_TYPE_RELATIONSHIP: &str = "relationship";
pub const STANDARD_TEST_TYPE_ACCEPTED_VALUES: &str = "accepted_values";
pub const STANDARD_TEST_TYPE_LESS_THAN_OR_EQUAL: &str = "lte";
pub const STANDARD_TEST_TYPE_GREATER_THAN_OR_EQUAL: &str = "gte";
pub const STANDARD_TEST_TYPE_LESS_THAN: &str = "lt";
pub const STANDARD_TEST_TYPE_GREATER_THAN: &str = "gt";

#[cfg(test)]
mod tests {
    use super::*;
    use quary_proto::project_file::{Column, Model, Source};
    use std::io::Cursor;

    #[test]
    fn test_serialize_deserialize_project_file() {
        let project_file = ProjectFile {
            models: vec![Model {
                name: "model test".to_string(),
                description: Some("test description for model".to_string()),
                columns: vec![Column {
                    name: "column test".to_string(),
                    description: Some("test description for column".to_string()),
                    tests: vec![],
                }],
            }],
            sources: vec![Source {
                name: "source_test".to_string(),
                description: Some("test description for source".to_string()),
                path: "source_test.source_test".to_string(),
                columns: vec![Column {
                    name: "column test".to_string(),
                    description: Some("test description for sources column".to_string()),
                    tests: vec![],
                }],
            }],
        };

        let yaml_str = serialize_project_file_to_yaml(project_file.clone()).unwrap();

        let deserialized_project_file =
            deserialize_project_file_from_yaml(Box::new(Cursor::new(yaml_str))).unwrap();

        assert_eq!(
            project_file, deserialized_project_file,
            "Deserialized object does not match the original"
        );
    }
}
