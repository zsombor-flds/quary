use crate::automatic_branching::derive_sha256_file_contents;
use crate::databases::DatabaseQueryGenerator;
use crate::file_system::FileSystem;
use crate::graph::{project_to_graph, Edge};
use crate::map_helpers::safe_adder_map;
use crate::models::{parse_model_schemas_to_views, read_normalise_model};
use crate::project_file::deserialize_project_file_from_yaml;
use crate::schema_name::DEFAULT_SCHEMA_PREFIX;
use crate::seeds::parse_table_schema_seeds;
use crate::sql::{remove_sql_comments, return_reference_search};
use crate::sql_inference_translator::unprefix_model;
use crate::test_helpers::ToTest;
use crate::tests::test_to_name;
use quary_proto::model::ModelColum;
use quary_proto::test::TestType::{
    AcceptedValues, GreaterThan, GreaterThanOrEqual, LessThan, LessThanOrEqual, NotNull,
    Relationship, Sql, Unique,
};
use quary_proto::{
    Model, Project, ProjectFile, Seed, Source, Test, TestAcceptedValues, TestGreaterThan,
    TestGreaterThanOrEqual, TestLessThan, TestLessThanOrEqual, TestNotNull, TestRelationship,
    TestSqlFile, TestUnique,
};
use sqlinference::infer_tests::{get_column_with_source, ExtractedSelect};
use sqlparser::dialect::Dialect;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};

/// build_column_description_map returns a map of column name in the sql statement to a column
/// description from the project of an underlying source that particular column refers if it
/// refers to the column directly, i.e., not through an operation.
///
/// For example, suppose
/// - the project has a model called 'model_b' with a column 'column_b' that has a description 'description of column_b'
/// - the sql statement is 'SELECT column_b AS column_a FROM model_a'
///
/// Then the returned map would be:
/// {
///    "column_a": "description of column_b"
/// }
pub fn build_column_description_map(
    project: &Project,
    model_sql: &str,
    modelling_prefix: &str,
    dialect: &dyn Dialect,
) -> Result<HashMap<String, String>, String> {
    let extracted = get_column_with_source(dialect, model_sql);
    match extracted {
        Ok(extracted) => match extracted {
            ExtractedSelect::Extracted { mapped, .. } => Ok(mapped
                .into_iter()
                .filter_map(|(target, (source_reference, source_column))| {
                    #[allow(clippy::unwrap_used)]
                    let target_model = unprefix_model(&source_reference, modelling_prefix).unwrap();
                    let model = project.models.get(&target_model)?;
                    let column = model.columns.iter().find(|m| m.title == source_column)?;
                    column
                        .description
                        .as_ref()
                        .map(|description| (target, description.clone()))
                })
                .collect()),
            // TODO This can be smarter
            ExtractedSelect::Star(_) => Ok(HashMap::new()),
        },
        Err(_) => Ok(HashMap::new()),
    }
}

/// build_column_description_map_for_model does the same as build_column_description_map but rather
/// than taking a sql statement it takes a model name, reads the sql for that model and infers a
/// description map like 'build_column_description_map' and returns the map for that model.
pub fn build_column_description_map_for_model(
    project: &Project,
    modelling_prefix: &str,
    model: &str,
    dialect: &dyn Dialect,
    file_system: &impl FileSystem,
) -> Result<HashMap<String, String>, String> {
    let model = project
        .models
        .get(model)
        .ok_or(format!("could not find model {}", model))?;
    let model_sql_code = file_system
        .read_file(&model.file_path)
        .map_err(|e| format!("could not read model file {}: {}", model.file_path, e))?;
    let model_sql_code = read_normalise_model(Box::new(model_sql_code))?;
    build_column_description_map(project, &model_sql_code, modelling_prefix, dialect)
}

/// return_defined_description_map returns a map of column name to description for that particular
/// column that is defined in the project. This is different from 'build_column_description_map' in
/// that it returns only defined descriptions from project files.
pub fn return_defined_description_map<'a>(
    project: &'a Project,
    model_name: &str,
) -> Option<HashMap<&'a str, &'a str>> {
    let model_hash_map = project.models.get(model_name).map(|model| {
        model
            .columns
            .iter()
            .filter_map(|column| {
                column
                    .description
                    .as_ref()
                    .map(|description| (column.title.as_str(), description.as_str()))
            })
            .collect::<HashMap<&str, &str>>()
    });
    if model_hash_map.is_some() {
        return model_hash_map;
    };
    let source_hash_map = project.sources.get(model_name).map(|source| {
        source
            .columns
            .iter()
            .filter_map(|column| {
                column
                    .description
                    .as_ref()
                    .map(|description| (column.title.as_str(), description.as_str()))
            })
            .collect::<HashMap<&str, &str>>()
    });
    if source_hash_map.is_some() {
        return source_hash_map;
    };
    None
}

/// return_tests_for_a_particular_model returns an iterator of tests for a particular model specified.
pub fn return_tests_for_a_particular_model<'a>(
    project: &'a Project,
    model: &'a str,
) -> impl Iterator<Item = &'a Test> {
    project
        .tests
        .iter()
        .filter(move |(_, test)| match &test.test_type {
            None => false,
            Some(Sql(_)) => false,
            Some(NotNull(test)) => test.model == model,
            Some(Unique(test)) => test.model == model,
            Some(AcceptedValues(test)) => test.model == model,
            Some(Relationship(test)) => test.source_model == model,
            Some(GreaterThanOrEqual(test)) => test.model == model,
            Some(LessThanOrEqual(test)) => test.model == model,
            Some(LessThan(test)) => test.model == model,
            Some(GreaterThan(test)) => test.model == model,
        })
        .map(|(_, test)| test)
}

/// ParseProject parses a whole project into a project object.
pub fn parse_project(
    filesystem: &impl FileSystem,
    database: &impl DatabaseQueryGenerator,
    project_root: &str,
) -> Result<Project, String> {
    let seeds = parse_seeds(filesystem, project_root)?
        .into_iter()
        .collect::<HashMap<_, _>>();
    let project_files = parse_project_files(filesystem, project_root)?;
    let sources = parse_sources(&project_files).collect::<HashMap<_, _>>();

    // TODO: Think about implementing custom tests
    // let custom_tests = parse_custom_tests(&filesystem, &project_root)?;

    let all_models: Vec<quary_proto::project_file::Model> = project_files
        .iter()
        .flat_map(|(_, project_file)| project_file.models.clone())
        .collect();
    let model_definitions = all_models
        .iter()
        .map(|m| (m.name.clone(), m.clone()))
        .collect();
    let models = parse_models(filesystem, project_root, &model_definitions)?;

    let path_map = create_path_map(
        models.values().collect::<Vec<&Model>>().clone(),
        sources.values().collect::<Vec<&Source>>().clone(),
    );
    let tests = parse_tests(
        filesystem,
        database,
        project_root,
        &path_map,
        &project_files,
    )?;

    // Check that models all refer to actual models
    for model in models.values() {
        for reference in &model.references {
            if !models.contains_key(reference)
                && !sources.contains_key(reference)
                && !seeds.contains_key(reference)
            {
                return Err(format!(
                    "model {:?} has reference to {:?} which is not a model or source",
                    model.name, reference
                ));
            }
        }
    }

    // Check that all tests refer to actual models
    for test in tests.values() {
        if let Some(Sql(sql)) = &test.test_type {
            for reference in &sql.references {
                if !models.contains_key(reference)
                    && !sources.contains_key(reference)
                    && !seeds.contains_key(reference)
                {
                    return Err(format!(
                        "test {:?} has reference to {:?} which is not a model or source",
                        test, reference
                    ));
                }
            }
        }
    }

    Ok(Project {
        seeds,
        models: models.into_iter().collect(),
        sources,
        tests: tests.into_iter().collect(),
        project_files,
    })
}

fn parse_tests(
    filesystem: &impl FileSystem,
    database: &impl DatabaseQueryGenerator,
    project_root: &str,
    path_map: &PathMap,
    project_files: &HashMap<String, ProjectFile>,
) -> Result<BTreeMap<String, Test>, String> {
    let sql_tests = parse_sql_tests(filesystem, project_root)?;

    let column_tests = parse_column_tests(database, project_files, path_map)?;

    let mut tests = BTreeMap::<String, Test>::new();
    for (name, test) in sql_tests {
        safe_adder_map(&mut tests, name, test)?;
    }
    for (name, test) in column_tests {
        safe_adder_map(&mut tests, name, test)?;
    }
    Ok(tests)
}

fn parse_sql_tests(
    file_system: &impl FileSystem,
    project_root: &str,
) -> Result<HashMap<String, Test>, String> {
    let paths = get_path_bufs(file_system, project_root, PATH_FOR_TESTS, EXTENSION_SQL)?;

    let reference_search =
        return_reference_search(DEFAULT_SCHEMA_PREFIX).map_err(|e| e.to_string())?;

    paths
        .iter()
        .map(|path| {
            let path = path
                .to_str()
                .ok_or(format!("Could not parse test file path: {:?}", path))?;
            let mut file = file_system
                .read_file(path)
                .map_err(|err| format!("failed to read file: {:?}", err))?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)
                .map_err(|err| format!("failed to read to string: {:?}", err))?;
            let contents = remove_sql_comments(&contents);
            let mut references = reference_search
                .captures_iter(&contents)
                .map(|cap| {
                    cap.iter()
                        .map(|m| {
                            let m = m.ok_or(format!(
                                "Could not parse reference search from schema name: {:?}",
                                m
                            ))?;
                            Ok(m.as_str().to_string())
                        })
                        .skip(1)
                        .step_by(2)
                        .collect::<Result<Vec<_>, String>>()
                })
                .collect::<Result<Vec<_>, String>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<String>>();
            references.sort();

            let file_path = path.to_string();

            let test = TestSqlFile {
                file_path,
                references,
            };
            let name = test_to_name(&test.to_test())?;

            Ok((name, test.to_test()))
        })
        .collect()
}

fn create_path_map(models: Vec<&Model>, sources: Vec<&Source>) -> PathMap {
    let model_entries = models
        .iter()
        .map(|model| (model.name.to_string(), model.name.to_string()));
    let source_entries = sources
        .iter()
        .map(|source| (source.name.to_string(), source.path.to_string()));
    model_entries.chain(source_entries).collect()
}

fn get_path_bufs(
    filesystem: &impl FileSystem,
    project_root: &str,
    folder: &str,
    extension_of_interest: &str,
) -> Result<Vec<PathBuf>, String> {
    let mut out = PathBuf::from(project_root);
    out.push(folder);
    let path = out.to_str().ok_or(format!(
        "Could not parse path to string: {:?}",
        out.to_str()
    ))?;

    filesystem
        .list_all_files_recursively(path)?
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<PathBuf>>()
        .iter()
        .filter(|path: &&PathBuf| path.extension() == Some(OsStr::new(extension_of_interest)))
        .map(|path| Ok(path.clone()))
        .collect()
}

fn parse_models(
    filesystem: &impl FileSystem,
    project_root: &str,
    model_definitions: &ModelDefinition,
) -> Result<BTreeMap<String, Model>, String> {
    let paths = get_path_bufs(filesystem, project_root, PATH_FOR_MODELS, EXTENSION_SQL)?;

    let models = paths
        .iter()
        .map(|path| {
            parse_model(
                filesystem,
                model_definitions,
                path.to_str()
                    .ok_or(format!("Could not parse model path: {:?}", path))?,
            )
        })
        .collect::<Result<Vec<Model>, String>>()?;

    let mut model_map = BTreeMap::<String, Model>::new();
    for model in models {
        safe_adder_map(&mut model_map, model.name.clone(), model)?;
    }

    Ok(model_map)
}

pub type ModelDefinition = HashMap<String, quary_proto::project_file::Model>;

fn parse_model(
    file_system: &impl FileSystem,
    model_definitions: &ModelDefinition,
    sql_path: &str,
) -> Result<Model, String> {
    let path_buf = Path::new(sql_path);
    let name = path_buf.file_stem().ok_or(format!(
        "Could not parse model name from path: {:?}",
        path_buf
    ))?;
    let name = name
        .to_str()
        .ok_or(format!(
            "Could not parse model name from path: {:?}",
            path_buf
        ))?
        .to_string();

    let description = model_definitions
        .get(&name)
        .map(|model| model.description.clone())
        .unwrap_or(None);
    let reference_search = return_reference_search(DEFAULT_SCHEMA_PREFIX)
        .map_err(|e| format!("Could not parse reference search from schema name: {:?}", e))?;

    let mut file = file_system
        .read_file(sql_path)
        .map_err(|e| format!("failed to read file: {:?}", e))?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| format!("failed to read string: {:?}", e))?;
    let contents = remove_sql_comments(&contents);
    let mut references: Vec<String> = reference_search
        .captures_iter(&contents)
        .map(|cap| {
            cap.iter()
                .map(|m| {
                    Ok(m.ok_or(format!(
                        "Could not parse reference search from schema name: {:?}",
                        m
                    ))?
                    .as_str()
                    .to_string())
                })
                .skip(1)
                .step_by(2)
                .collect::<Result<Vec<_>, String>>()
        })
        .collect::<Result<Vec<Vec<_>>, String>>()?
        .into_iter()
        .flatten()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    references.sort();

    let empty = quary_proto::project_file::Model::default();
    let columns = model_definitions
        .get(&name)
        .unwrap_or(&empty)
        .columns
        .iter()
        .map(|column| ModelColum {
            title: column.name.to_string(),
            description: column.description.clone(),
        })
        .collect();

    let file_sha256_hash = derive_sha256_file_contents(file_system, sql_path)?;

    if references.contains(&name) {
        return Err(format!("model {} has a reference to itself", name));
    }

    Ok(Model {
        name,
        description,
        file_sha256_hash,
        file_path: sql_path.to_string(),
        columns,
        references,
    })
}

fn parse_seeds<'a>(
    filesystem: &'a impl FileSystem,
    project_root: &'a str,
) -> Result<Vec<(String, Seed)>, String> {
    let paths = get_path_bufs(filesystem, project_root, PATH_FOR_SEEDS, EXTENSION_CSV)?;

    paths
        .iter()
        .map(|path| {
            let name = path
                .file_stem()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
                .ok_or(format!("Could not parse seed name from path: {:?}", path))?;
            let path = path
                .to_str()
                .ok_or(format!("Could not parse seed path: {:?}", path))?;
            let file_sha256_hash = derive_sha256_file_contents(filesystem, path)?;
            Ok((
                name.clone(),
                Seed {
                    name,
                    file_sha256_hash,
                    file_path: path.to_string(),
                },
            ))
        })
        .collect()
}

pub fn parse_project_files(
    filesystem: &impl FileSystem,
    project_root: &str,
    // Map of full path to project file and project file
) -> Result<HashMap<String, ProjectFile>, String> {
    let paths = get_path_bufs(filesystem, project_root, PATH_FOR_MODELS, EXTENSION_YAML)?;

    paths
        .iter()
        .map(|path| {
            let path_str = path
                .to_str()
                .ok_or_else(|| format!("Could not parse project file path: {:?}", path))?
                .to_string(); // Convert &str to String here
            let project_file_contents = filesystem
                .read_file(&path_str)
                .map_err(|e| format!("Could not read project file '{}': {:?}", path_str, e))?;
            let project_file: ProjectFile =
                deserialize_project_file_from_yaml(project_file_contents)
                    .map_err(|e| format!("Could not parse project file '{}': {:?}", path_str, e))?;
            Ok((path_str, project_file))
        })
        .collect::<Result<HashMap<String, ProjectFile>, String>>()
}

/// Returns sources from project files
fn parse_sources(
    project_files: &'_ HashMap<String, ProjectFile>,
) -> impl Iterator<Item = (String, Source)> + '_ {
    project_files.iter().flat_map(|(path, project_file)| {
        project_file.sources.iter().map(|s| {
            (
                s.name.to_string(),
                Source {
                    name: s.name.to_string(),
                    description: s.description.clone(),
                    // TODO This needs to become a location
                    path: s.path.to_string(),
                    file_path: path.to_string(),
                    // TODO Map columns or remove them from this model
                    columns: vec![],
                },
            )
        })
    })
}

type PathMap = HashMap<String, String>;

fn parse_column_tests(
    database: &impl DatabaseQueryGenerator,
    project_files: &HashMap<String, ProjectFile>,
    path_map: &PathMap,
) -> Result<HashMap<String, Test>, String> {
    let mut outs = HashMap::<String, Test>::new();

    for (file_path, project_file) in project_files {
        for source in &project_file.sources {
            for column in &source.columns {
                let tests = parse_column_tests_for_model_or_source(
                    column,
                    path_map,
                    file_path,
                    &source.name,
                    &source.path,
                )?;
                for (name, test) in tests {
                    safe_adder_map(&mut outs, name, test)?;
                }
            }
        }
        for model in &project_file.models {
            let model_path = database.return_full_path_requirement(&model.name);
            for column in &model.columns {
                let tests = parse_column_tests_for_model_or_source(
                    column,
                    path_map,
                    file_path,
                    &model.name,
                    model_path.as_str(),
                )?;
                for (name, test) in tests {
                    safe_adder_map(&mut outs, name, test)?;
                }
            }
        }
    }

    Ok(outs)
}

// TODO Move this close to the other file which is the same in reverse in rpc_proto_defined
fn parse_column_tests_for_model_or_source(
    column: &quary_proto::project_file::Column,
    path_map: &PathMap,
    file_path: &str,
    model_name: &str,
    model_path: &str,
) -> Result<HashMap<String, Test>, String> {
    column
        .tests
        .iter()
        .map(|test| match test.r#type.as_str() {
            crate::project_file::STANDARD_TEST_TYPE_SQL_NOT_NULL => {
                let test = TestNotNull {
                    file_path: file_path.to_string(),
                    model: model_name.to_string(),
                    path: model_path.to_string(),
                    column: column.name.to_string(),
                }
                .to_test();
                let name = test_to_name(&test)?;
                Ok((name, test))
            }
            crate::project_file::STANDARD_TEST_TYPE_SQL_UNIQUE => {
                let test = TestUnique {
                    file_path: file_path.to_string(),
                    model: model_name.to_string(),
                    path: model_path.to_string(),
                    column: column.name.to_string(),
                }
                .to_test();
                let name = test_to_name(&test)?;
                Ok((name, test))
            }
            crate::project_file::STANDARD_TEST_TYPE_ACCEPTED_VALUES => {
                let values = test
                    .info
                    .get("values")
                    .ok_or(format!("test {:?} is missing values", test))?
                    .split(',')
                    .map(|s| s.to_string())
                    .collect();
                let test = TestAcceptedValues {
                    file_path: file_path.to_string(),
                    model: model_name.to_string(),
                    path: model_path.to_string(),
                    column: column.name.to_string(),
                    accepted_values: values,
                }
                .to_test();
                let name = test_to_name(&test)?;
                Ok((name, test))
            }
            crate::project_file::STANDARD_TEST_TYPE_RELATIONSHIP => {
                let info = &test.info;
                let target_column = info
                    .get("column")
                    .ok_or(format!("test {:?} is missing column", test))?;
                let target_model = info
                    .get("model")
                    .ok_or(format!("test {:?} is missing model", test))?;
                let test = TestRelationship {
                    file_path: file_path.to_string(),
                    source_model: model_name.to_string(),
                    source_path: model_path.to_string(),
                    source_column: column.name.to_string(),
                    target_model: target_model.to_string(),
                    target_path: path_map
                        .get(target_model)
                        .ok_or(format!(
                            "test {:?} has unknown target model in {:?}",
                            test, path_map
                        ))?
                        .to_string(),
                    target_column: target_column.to_string(),
                }
                .to_test();
                let name = test_to_name(&test)?;
                Ok((name, test))
            }
            crate::project_file::STANDARD_TEST_TYPE_GREATER_THAN_OR_EQUAL => {
                let value = test
                    .info
                    .get("value")
                    .ok_or(format!("test {:?} is missing value", test))?;
                let test = TestGreaterThanOrEqual {
                    file_path: file_path.to_string(),
                    model: model_name.to_string(),
                    path: model_path.to_string(),
                    column: column.name.to_string(),
                    value: value.to_string(),
                }
                .to_test();
                let name = test_to_name(&test)?;
                Ok((name, test))
            }
            crate::project_file::STANDARD_TEST_TYPE_GREATER_THAN => {
                let value = test
                    .info
                    .get("value")
                    .ok_or(format!("test {:?} is missing value", test))?;
                let test = TestGreaterThan {
                    file_path: file_path.to_string(),
                    model: model_name.to_string(),
                    path: model_path.to_string(),
                    column: column.name.to_string(),
                    value: value.to_string(),
                }
                .to_test();
                let name = test_to_name(&test)?;
                Ok((name, test))
            }
            crate::project_file::STANDARD_TEST_TYPE_LESS_THAN_OR_EQUAL => {
                let value = test
                    .info
                    .get("value")
                    .ok_or(format!("test {:?} is missing value", test))?;
                let test = TestLessThanOrEqual {
                    file_path: file_path.to_string(),
                    model: model_name.to_string(),
                    path: model_path.to_string(),
                    column: column.name.to_string(),
                    value: value.to_string(),
                }
                .to_test();
                let name = test_to_name(&test)?;
                Ok((name, test))
            }
            crate::project_file::STANDARD_TEST_TYPE_LESS_THAN => {
                let value = test
                    .info
                    .get("value")
                    .ok_or(format!("test {:?} is missing value", test))?;
                let test = TestLessThan {
                    file_path: file_path.to_string(),
                    model: model_name.to_string(),
                    path: model_path.to_string(),
                    column: column.name.to_string(),
                    value: value.to_string(),
                }
                .to_test();
                let name = test_to_name(&test)?;
                Ok((name, test))
            }
            _ => Err(format!(
                "test {:?} has unknown type {:?}",
                test,
                test.r#type.as_str()
            )),
        })
        .collect()
}

/// overrides is a map of model name to the string that the reference should be replaced with if
/// a model is used that is found in overrides, then the reference will be replaced with a
/// `SELECT * FROM {found_value}` and any upstream references are dropped.
///
/// For example, if the dependencies are A -> B -> C and overrides is {B: "D"} then the returned
/// A -> D.
pub fn project_and_fs_to_query_sql(
    database: &impl DatabaseQueryGenerator,
    project: &Project,
    file_system: &impl FileSystem,
    model_name: &str,
    overrides: Option<HashMap<String, String>>,
) -> Result<(String, Vec<Edge>), String> {
    enum SourceModelSeed {
        Source,
        Seed,
        Model,
    }
    // TODO Some really ugly code here. Clean it up.
    match (
        project.sources.get(model_name),
        project.seeds.get(model_name),
        project.models.get(model_name),
    ) {
        (Some(_), None, None) => Ok(SourceModelSeed::Source),
        (None, Some(_), None) => Ok(SourceModelSeed::Seed),
        (None, None, Some(_)) => Ok(SourceModelSeed::Model),
        _ => Err(format!(
            "requested {:?} is neither a seed, nor source, nor model",
            model_name
        )),
    }?;

    let overrides = overrides.unwrap_or_default();
    let graph = project_to_graph(project.clone())?;
    let mut overriden_graph = graph.graph;
    for model in overrides.keys() {
        if overriden_graph.graph.node_weights().any(|n| n == model) {
            let (override_graph, _) =
                overriden_graph.return_shrunk_downstream_graph(model.as_str())?;
            overriden_graph = override_graph;
        }
    }

    let (upstream, models) = overriden_graph.return_parent_nods_to_apply_in_order(model_name)?;

    let to_process: Vec<NodeWithName> = models
        .iter()
        .map(|name| {
            match (
                overrides.get(name),
                project.sources.get(name),
                project.seeds.get(name),
                project.models.get(name),
            ) {
                (Some(overriden), None, None, Some(_)) => Ok(NodeWithName {
                    name: name.to_string(),
                    node: ModelOrSeed::Override((name.clone(), overriden.clone())),
                }),
                (None, Some(source), None, None) => Ok(NodeWithName {
                    name: name.to_string(),
                    node: ModelOrSeed::Source(source.clone()),
                }),
                (None, None, Some(seed), None) => Ok(NodeWithName {
                    name: name.to_string(),
                    node: ModelOrSeed::Seed(seed.clone()),
                }),
                (None, None, None, Some(model)) => Ok(NodeWithName {
                    name: name.to_string(),
                    node: ModelOrSeed::Model(model.clone()),
                }),
                _ => Err(format!(
                    "model {:?} is neither a seed nor a model nor a source nor a override",
                    name
                )),
            }
        })
        .collect::<Result<Vec<_>, String>>()?;

    let sql = convert_to_select_statement(database, file_system, &to_process)?;

    let edges = upstream.return_graph_edges()?;

    Ok((sql, edges))
}

/// project_and_fs_to_sql_for_views returns the sql for creating tables for seeds and views for models
/// for the project. The entries are returned in the right order in which they need to be applied.
/// Each returned entry is a tuple of the form (model/seed name, sql statements that make up the
/// model/seed).
///
/// returns: Result<Vec<(String, Vec<String, Global>), Global>, String>
pub fn project_and_fs_to_sql_for_views(
    project: &Project,
    file_system: &impl FileSystem,
    database: &impl DatabaseQueryGenerator,
    only_models: bool,
    do_not_include_seeds_data: bool,
) -> Result<Vec<(String, Vec<String>)>, String> {
    let graph = project_to_graph(project.clone())?;
    let sorted = graph.graph.get_node_sorted()?;

    let models = sorted
        .iter()
        .filter_map(|node| {
            match (
                project.seeds.get(node),
                project.sources.get(node),
                project.tests.get(node),
                project.models.get(node),
            ) {
                (Some(_), _, _, _) => None,
                (_, Some(_), _, _) => None,
                (_, _, Some(_), _) => None,
                (_, _, _, Some(model)) => Some(model),
                _ => None,
            }
        })
        .map(|model| {
            let file = file_system.read_file(&model.file_path).map_err(|e| {
                format!(
                    "failed to read file {:?} with error {:?}",
                    model.file_path, e
                )
            })?;
            let sql_view = parse_model_schemas_to_views(
                database,
                file,
                &model.name,
                DEFAULT_SCHEMA_PREFIX,
                |s| {
                    let replaced = replace_reference_string_found_with_database(
                        &project.sources,
                        &database,
                    )(s);
                    let replaced = replaced.trim();
                    format!(" {}", replaced)
                },
            )?;
            Ok((model.name.clone(), sql_view))
        })
        .collect::<Result<Vec<(String, [String; 2])>, String>>()?;

    let models_map: HashMap<String, Vec<String>> = models
        .iter()
        .map(|(name, vec)| (name.clone(), vec.to_vec()))
        .collect::<HashMap<String, Vec<String>>>();
    let mut models = vec![];
    for model in sorted.iter() {
        if let Some(sqls) = models_map.get(model) {
            models.push((model.clone(), sqls.clone()));
        }
    }
    // let mut order: Vec<(String, Vec<String>>) = Vec::new();
    // }
    // models.sort_by_key(|a| *order.get(&a.0).unwrap());

    // TODO Make more efficient by skipping the seeds if this is on.
    if only_models {
        return Ok(models);
    }
    let mut seeds: Vec<&Seed> = project.seeds.values().collect();
    seeds.sort_by_key(|a| a.name.clone());
    let mut seeds_out: Vec<(String, Vec<String>)> = seeds
        .iter()
        .map(|seed| {
            let reader = file_system.read_file(&seed.file_path).map_err(|e| {
                format!(
                    "failed to read file {:?} with error {:?}",
                    seed.file_path, e
                )
            })?;
            let values = parse_table_schema_seeds(
                database,
                &seed.name,
                reader,
                do_not_include_seeds_data,
            )
            .map_err(|e| format!("failed to parse seed {:?} with error {:?}", seed.name, e))?;
            Ok((seed.name.clone(), values))
        })
        .collect::<Result<Vec<(String, Vec<String>)>, String>>()?;

    seeds_out.append(&mut models);
    Ok(seeds_out)
}

/// convertToSelectStatements takes in an array of model/seed and returns a string that can be used in a select statement.
/// It also replaces any q.references with the actual name that is in the select. It uses no views.
///
/// array of models is in the shape of [][2]string where the first element is the name of the model and the second element is the sql
fn convert_to_select_statement(
    database: &impl DatabaseQueryGenerator,
    file_system: &impl FileSystem,
    values: &[NodeWithName],
) -> Result<String, String> {
    /// Info contains the name of the model and the sql for that model
    type Info = (String, String);

    let nodes = values
        .iter()
        .map(|node| match &node.node {
            ModelOrSeed::Override((name, target)) => {
                let sql = render_override_select_statement(target);
                Ok((name.clone(), sql))
            }
            ModelOrSeed::Source(source) => {
                let sql = render_source_select_statement(source);
                Ok((node.name.clone(), sql))
            }
            ModelOrSeed::Seed(seed) => {
                let sql = render_seed_select_statement(database, file_system, seed)?;
                Ok((node.name.clone(), sql))
            }
            ModelOrSeed::Model(model) => {
                let sql = render_model_select_statement(database, file_system, model)?;
                Ok((node.name.clone(), sql))
            }
        })
        .collect::<Result<Vec<Info>, String>>()?;

    match &nodes[..] {
        [] => Err("no nodes to process".to_string()),
        [(_, sql)] => Ok(sql.clone()),
        [node1, node2] => Ok(format!(
            "WITH {} AS ({}) SELECT * FROM ({})",
            node1.0, node1.1, node2.1
        )),
        [withs @ .., last] => {
            let withs = withs
                .iter()
                .map(|(name, sql)| format!("{} AS ({})", name, sql))
                .collect::<Vec<_>>()
                .join(",\n");
            Ok(format!("WITH\n{}\nSELECT * FROM ({})", withs, last.1))
        }
    }
}

fn render_seed_select_statement(
    database: &impl DatabaseQueryGenerator,
    fs: &impl FileSystem,
    seed: &Seed,
) -> Result<String, String> {
    let reader = fs.read_file(seed.file_path.as_str()).map_err(|e| {
        format!(
            "failed to read file {:?} with error {:?}",
            seed.file_path, e
        )
    })?;

    let mut csv_reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(reader);

    let mut records: Vec<Vec<String>> = Vec::new();
    for result in csv_reader.records() {
        let record = result
            .map(|v| v.iter().map(|s| s.to_string()).collect::<Vec<String>>())
            .map_err(|e| format!("error reading csv record: {:?}", e))?;
        records.push(record);
    }
    let headers = records
        .first()
        .ok_or("no headers found in csv".to_string())?;
    let records = records
        .get(1..)
        .ok_or("no records found in csv".to_string())?;

    Ok(render_seed_select_statement_string(
        database,
        headers.clone(),
        records.to_vec(),
    ))
}

fn render_seed_select_statement_string(
    database: &impl DatabaseQueryGenerator,
    headers: Vec<String>,
    values: Vec<Vec<String>>,
) -> String {
    let header_section = headers
        .iter()
        .enumerate()
        .map(|(i, header)| format!("column{} AS {}", i + 1, header))
        .collect::<Vec<String>>()
        // TODO Could use intersperse here
        .join(",");

    let values_section = values
        .iter()
        .map(|row| {
            format!(
                "({})",
                row.iter()
                    .map(|value| format!("'{}'", database.escape_seed_value(value)))
                    .collect::<Vec<String>>()
                    .join(",")
            )
        })
        .collect::<Vec<String>>()
        .join(",");

    format!("SELECT {} FROM (VALUES {})", header_section, values_section)
}

fn render_source_select_statement(source: &Source) -> String {
    format!("SELECT * FROM {}", source.path)
}

fn render_override_select_statement(override_target: &str) -> String {
    format!("SELECT * FROM {}", override_target)
}

fn render_model_select_statement(
    database: &impl DatabaseQueryGenerator,
    fs: &impl FileSystem,
    model: &Model,
) -> Result<String, String> {
    let reader = fs.read_file(model.file_path.as_str()).map_err(|e| {
        format!(
            "failed to read file {:?} with error {:?}",
            model.file_path, e
        )
    })?;
    let sql = read_normalise_model(reader)?;

    let reference_search = return_reference_search(DEFAULT_SCHEMA_PREFIX).map_err(|e| {
        format!(
            "error creating reference search for model {:?}: {:?}",
            model.name, e
        )
    })?;
    let replaced = reference_search.replace_all(
        sql.as_str(),
        replace_reference_string_found(&HashMap::new(), &database),
    );
    Ok(replaced.into_owned())
}

fn replace_reference_string_found_with_database<'a>(
    sources: &'a HashMap<String, Source>,
    database: &'a &impl DatabaseQueryGenerator,
) -> Box<dyn Fn(&regex::Captures) -> String + 'a> {
    #[allow(clippy::indexing_slicing)]
    Box::new(move |caps: &regex::Captures| {
        let model = &caps[1];
        let model = sources
            .get(model)
            .map(|s| s.path.clone())
            .unwrap_or(database.return_full_path_requirement(model));
        let wrapped = database.database_name_wrapper(model.as_str());
        format!(" {}", wrapped)
    })
}

pub fn replace_reference_string_found<'a>(
    overrides: &'a HashMap<String, String>,
    database: &'a &impl DatabaseQueryGenerator,
) -> Box<dyn Fn(&regex::Captures) -> String + 'a> {
    #[allow(clippy::indexing_slicing)]
    Box::new(move |caps: &regex::Captures| {
        let model = &caps[1];
        if let Some(path) = overrides.get(model) {
            if path.starts_with('(') & path.ends_with(')') {
                format!(" {}", path)
            } else {
                format!(" `{}`", path)
            }
        } else {
            format!(" {}", database.database_name_wrapper(model))
        }
    })
}

#[derive(Debug, Clone)]
enum ModelOrSeed {
    Model(Model),
    Seed(Seed),
    Source(Source),
    Override((String, String)),
}

#[derive(Debug, Clone)]
struct NodeWithName {
    name: String,
    node: ModelOrSeed,
}

const EXTENSION_CSV: &str = "csv";
const EXTENSION_YAML: &str = "yaml";
const EXTENSION_SQL: &str = "sql";

pub(crate) const PATH_FOR_SEEDS: &str = "seeds";
pub const PATH_FOR_MODELS: &str = "models";
pub(crate) const PATH_FOR_TESTS: &str = "tests";

#[cfg(test)]
mod test {
    use super::*;
    use crate::database_bigquery::DatabaseQueryGeneratorBigQuery;
    use crate::database_sqlite::DatabaseQueryGeneratorSqlite;
    use crate::init::Asset;

    #[test]
    fn test_return_tests_for_a_particular_model() {
        let assets = Asset {};

        let project = parse_project(&assets, &DatabaseQueryGeneratorSqlite::default(), "").unwrap();

        let tests: Vec<_> =
            return_tests_for_a_particular_model(&project, "shifts_by_month").collect();

        assert_eq!(tests.len(), 5);
    }

    #[test]
    fn test_render_seed_select_statement_string() {
        let headers = vec!["id".to_string(), "name".to_string()];
        let values = vec![
            vec!["1".to_string(), "Bob's Burger".to_string()],
            vec!["2".to_string(), "Sally".to_string()],
        ];
        let expected =
            "SELECT column1 AS id,column2 AS name FROM (VALUES (\'1\',\'Bob\'\'s Burger\'),(\'2\',\'Sally\'))";
        let database = DatabaseQueryGeneratorSqlite::default();
        let actual = render_seed_select_statement_string(&database, headers, values);
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_render_seed_select_statement() {
        let fs = quary_proto::FileSystem {
            files: vec![(
                "test.csv".to_string(),
                quary_proto::File {
                    name: "test.csv".to_string(),
                    contents: prost::bytes::Bytes::from("id,name\n1,Bob\n2,Sally".to_string()),
                },
            )]
            .into_iter()
            .collect::<HashMap<String, quary_proto::File>>(),
        };

        let seed = Seed {
            name: "test".to_string(),
            file_path: "test.csv".to_string(),
            file_sha256_hash: "test".to_string(),
        };

        let expected =
            "SELECT column1 AS id,column2 AS name FROM (VALUES (\'1\',\'Bob\'),(\'2\',\'Sally\'))";
        let database = DatabaseQueryGeneratorSqlite::default();
        let actual = render_seed_select_statement(&database, &fs, &seed).unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_parse_model() {
        let sql = "WITH shifts AS (SELECT employee_id,
                            shift_date,
                            shift
                            FROM q.stg_shifts
            ),
            shift_details AS (SELECT shift AS shift_name,
                              start_time,
                              end_time
                              FROM q.shift_hours
            )
            shift_not_used AS (SELECT shift AS shift_name,
                              start_time,
                              end_time
                              FROM q.shift_hours
            )

            SELECT s.employee_id AS employee_id,
            s.shift AS shift,
            datetime(s.shift_date, sd.start_time) AS shift_start,
            datetime(s.shift_date, sd.end_time)   AS shift_end
            FROM shifts s
            INNER JOIN shift_details sd
            ON s.shift = sd.shift_name
    "
        .to_string();
        let file_system = quary_proto::FileSystem {
            files: vec![(
                "models/test.sql".to_string(),
                quary_proto::File {
                    name: "models/test.sql".to_string(),
                    contents: prost::bytes::Bytes::from(sql),
                },
            )]
            .into_iter()
            .collect(),
        };
        let model_definitions = vec![quary_proto::project_file::Model {
            name: "test".to_string(),
            description: None,
            columns: vec![],
        }]
        .into_iter()
        .map(|m| (m.name.clone(), m))
        .collect::<HashMap<String, _>>() as ModelDefinition;

        let model = parse_model(&file_system, &model_definitions, "models/test.sql").unwrap();

        assert_eq!(model.name, "test");
        assert_eq!(model.file_path, "models/test.sql");
        // Make sure no duplicates and in alphabetical order
        assert_eq!(model.references.len(), 2);
        assert_eq!(model.references[0], "shift_hours");
        assert_eq!(model.references[1], "stg_shifts");
    }

    #[test]
    fn test_parse_project_on_init() {
        let database = DatabaseQueryGeneratorSqlite::default();

        let assets = Asset {};
        let project = parse_project(&assets, &database, "").unwrap();

        assert!(!project.models.is_empty());
        assert!(project.models.contains_key("shifts"));
    }

    #[test]
    fn test_project_and_fs_to_query_sql_sqlite_simple_model_source() {
        let database = DatabaseQueryGeneratorSqlite::default();
        let fs = quary_proto::FileSystem {
            files: HashMap::from([
                (
                    "models/shifts.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts.sql".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "SELECT employee_id, shift_date, shift FROM q.raw_shifts",
                        ),
                    },
                ),
                (
                    "models/schema.yaml".to_string(),
                    quary_proto::File {
                        name: "models/schema.yaml".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "sources: [{name: raw_shifts, path: raw_shifts_real_table}]",
                        ),
                    },
                ),
            ]),
        };

        let project = parse_project(&fs, &database, "").unwrap();

        let (sql, _) =
            project_and_fs_to_query_sql(&database, &project, &fs, "shifts", None).unwrap();

        assert_eq!(sql, "WITH raw_shifts AS (SELECT * FROM raw_shifts_real_table) SELECT * FROM (SELECT employee_id, shift_date, shift FROM `raw_shifts`)");
    }

    #[test]
    fn test_project_and_fs_to_query_sql_sqlite_simple_model_model_source() {
        let database = DatabaseQueryGeneratorSqlite::default();
        let fs = quary_proto::FileSystem {
            files: HashMap::from([
                (
                    "models/shifts.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts.sql".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "SELECT employee_id, shift_date, shift FROM q.raw_shifts",
                        ),
                    },
                ),
                (
                    "models/shifts_transformed.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts_transformed.sql".to_string(),
                        contents: prost::bytes::Bytes::from("SELECT * FROM q.shifts"),
                    },
                ),
                (
                    "models/schema.yaml".to_string(),
                    quary_proto::File {
                        name: "models/schema.yaml".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "sources: [{name: raw_shifts, path: raw_shifts_real_table}]",
                        ),
                    },
                ),
            ]),
        };

        let project = parse_project(&fs, &database, "").unwrap();

        let (sql, _) =
            project_and_fs_to_query_sql(&database, &project, &fs, "shifts_transformed", None)
                .unwrap();

        assert_eq!(sql, "WITH\nraw_shifts AS (SELECT * FROM raw_shifts_real_table),\nshifts AS (SELECT employee_id, shift_date, shift FROM `raw_shifts`)\nSELECT * FROM (SELECT * FROM `shifts`)");
    }

    #[test]
    fn test_project_and_fs_to_query_sql_sqlite_simple_model_model_source_with_overides() {
        let database = DatabaseQueryGeneratorSqlite::default();
        let fs = quary_proto::FileSystem {
            files: HashMap::from([
                (
                    "models/shifts.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts.sql".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "SELECT employee_id, shift_date, shift FROM q.raw_shifts",
                        ),
                    },
                ),
                (
                    "models/shifts_transformed.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts_transformed.sql".to_string(),
                        contents: prost::bytes::Bytes::from("SELECT * FROM q.shifts"),
                    },
                ),
                (
                    "models/schema.yaml".to_string(),
                    quary_proto::File {
                        name: "models/schema.yaml".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "sources: [{name: raw_shifts, path: raw_shifts_real_table}]",
                        ),
                    },
                ),
            ]),
        };

        let project = parse_project(&fs, &database, "").unwrap();

        let (sql, _) = project_and_fs_to_query_sql(
            &database,
            &project,
            &fs,
            "shifts_transformed",
            Some(HashMap::from([
                ("shifts".to_string(), "qqq_shifts_hash".to_string()),
                (
                    "doesntexist".to_string(),
                    "qqq_doesntexist_hash".to_string(),
                ),
            ])),
        )
        .unwrap();

        assert_eq!(
            sql,
            "WITH shifts AS (SELECT * FROM qqq_shifts_hash) SELECT * FROM (SELECT * FROM `shifts`)"
        );
    }

    #[test]
    fn test_project_and_fs_to_query_sql_sqlite_simple_model_model_source_with_overide_end() {
        let database = DatabaseQueryGeneratorSqlite::default();
        let fs = quary_proto::FileSystem {
            files: HashMap::from([
                (
                    "models/shifts.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts.sql".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "SELECT employee_id, shift_date, shift FROM q.raw_shifts",
                        ),
                    },
                ),
                (
                    "models/shifts_transformed.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts_transformed.sql".to_string(),
                        contents: prost::bytes::Bytes::from("SELECT * FROM q.shifts"),
                    },
                ),
                (
                    "models/schema.yaml".to_string(),
                    quary_proto::File {
                        name: "models/schema.yaml".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "sources: [{name: raw_shifts, path: raw_shifts_real_table}]",
                        ),
                    },
                ),
            ]),
        };

        let project = parse_project(&fs, &database, "").unwrap();

        let (sql, _) = project_and_fs_to_query_sql(
            &database,
            &project,
            &fs,
            "shifts_transformed",
            Some(HashMap::from([(
                "shifts_transformed".to_string(),
                "qqq_shifts_transformed_hash".to_string(),
            )])),
        )
        .unwrap();

        assert_eq!(sql, "SELECT * FROM qqq_shifts_transformed_hash");
    }

    #[test]
    fn test_project_and_fs_to_query_sql_big_query_simple_model_source() {
        let database = DatabaseQueryGeneratorBigQuery::new(
            "test-project".to_string(),
            "test-dataset".to_string(),
        );
        let fs = quary_proto::FileSystem {
            files: HashMap::from([
                (
                    "models/shifts.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts.sql".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "SELECT employee_id, shift_date, shift FROM q.raw_shifts",
                        ),
                    },
                ),
                (
                    "models/schema.yaml".to_string(),
                    quary_proto::File {
                        name: "models/schema.yaml".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "sources: [{name: raw_shifts, path: test-project.test-dataset-2.raw_shifts_real_table}]",
                        ),
                    },
                ),
            ]),
        };

        let project = parse_project(&fs, &database, "").unwrap();

        let (sql, _) =
            project_and_fs_to_query_sql(&database, &project, &fs, "shifts", None).unwrap();

        // TODO: figure out if we should also use backticks for the table name here
        assert_eq!(sql, "WITH raw_shifts AS (SELECT * FROM test-project.test-dataset-2.raw_shifts_real_table) SELECT * FROM (SELECT employee_id, shift_date, shift FROM `raw_shifts`)");
    }

    #[test]
    fn test_project_and_fs_to_query_sql_big_query_simple_model_model_source() {
        let database = DatabaseQueryGeneratorBigQuery::new(
            "test-project".to_string(),
            "test-dataset".to_string(),
        );
        let fs = quary_proto::FileSystem {
            files: HashMap::from([
                (
                    "models/shifts.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts.sql".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "SELECT employee_id, shift_date, shift FROM q.raw_shifts",
                        ),
                    },
                ),
                (
                    "models/shifts_transformed.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts_transformed.sql".to_string(),
                        contents: prost::bytes::Bytes::from("SELECT * FROM q.shifts"),
                    },
                ),
                (
                    "models/schema.yaml".to_string(),
                    quary_proto::File {
                        name: "models/schema.yaml".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "sources: [{name: raw_shifts, path: test-project.test-dataset-2.raw_shifts_real_table}]",
                        ),
                    },
                ),
            ]),
        };

        let project = parse_project(&fs, &database, "").unwrap();

        let (sql, _) =
            project_and_fs_to_query_sql(&database, &project, &fs, "shifts_transformed", None)
                .unwrap();

        assert_eq!(sql, "WITH\nraw_shifts AS (SELECT * FROM test-project.test-dataset-2.raw_shifts_real_table),\nshifts AS (SELECT employee_id, shift_date, shift FROM `raw_shifts`)\nSELECT * FROM (SELECT * FROM `shifts`)");
    }

    #[test]
    fn test_project_and_fs_to_query_sql_big_query_simple_model_model_source_with_override() {
        let database = DatabaseQueryGeneratorBigQuery::new(
            "test-project".to_string(),
            "test-dataset".to_string(),
        );
        let fs = quary_proto::FileSystem {
            files: HashMap::from([
                (
                    "models/shifts.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts.sql".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "SELECT employee_id, shift_date, shift FROM q.raw_shifts",
                        ),
                    },
                ),
                (
                    "models/shifts_transformed.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts_transformed.sql".to_string(),
                        contents: prost::bytes::Bytes::from("SELECT * FROM q.shifts"),
                    },
                ),
                (
                    "models/schema.yaml".to_string(),
                    quary_proto::File {
                        name: "models/schema.yaml".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "sources: [{name: raw_shifts, path: test-project.test-dataset-2.raw_shifts_real_table}]",
                        ),
                    },
                ),
            ]),
        };

        let project = parse_project(&fs, &database, "").unwrap();

        let (sql, _) = project_and_fs_to_query_sql(
            &database,
            &project,
            &fs,
            "shifts_transformed",
            Some(HashMap::from([
                (
                    "shifts".to_string(),
                    "test-project.test-dataset-2.qqq_shifts_hash".to_string(),
                ),
                (
                    "doesntexist".to_string(),
                    "qqq_doesntexist_hash".to_string(),
                ),
            ])),
        )
        .unwrap();

        assert_eq!(sql, "WITH shifts AS (SELECT * FROM test-project.test-dataset-2.qqq_shifts_hash) SELECT * FROM (SELECT * FROM `shifts`)");
    }

    #[test]
    fn test_project_and_fs_to_sql_for_views_big_query() {
        let database =
            DatabaseQueryGeneratorBigQuery::new("quarylabs".to_string(), "transform".to_string());
        let fs = quary_proto::FileSystem {
            files: HashMap::from([
                (
                    "models/stg_commits.sql".to_string(),
                    quary_proto::File {
                        name: "models/stg_commits.sql".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "SELECT author FROM q.raw_commits",
                        ),
                    },
                ),
                (
                    "models/commits_transformed.sql".to_string(),
                    quary_proto::File {
                        name: "models/commits_transformed.sql".to_string(),
                        contents: prost::bytes::Bytes::from("SELECT * FROM q.stg_commits"),
                    },
                ),
                (
                    "models/schema.yaml".to_string(),
                    quary_proto::File {
                        name: "models/schema.yaml".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "sources: [{name: raw_commits, path: quarylabs.airbyte_github.commits}]",
                        ),
                    },
                ),
            ]),
        };
        let expected_output = vec![
            (
                "stg_commits".to_string(), 
                vec![
                    "DROP VIEW IF EXISTS `quarylabs.transform.stg_commits`".to_string(), 
                    "CREATE VIEW `quarylabs.transform.stg_commits` AS SELECT author FROM `quarylabs.airbyte_github.commits`".to_string()
                ]
            ),
            (
                "commits_transformed".to_string(), 
                vec![
                    "DROP VIEW IF EXISTS `quarylabs.transform.commits_transformed`".to_string(), 
                    "CREATE VIEW `quarylabs.transform.commits_transformed` AS SELECT * FROM `quarylabs.transform.stg_commits`".to_string()
                ]
            )
        ];

        let project = parse_project(&fs, &database, "").unwrap();
        let sql = project_and_fs_to_sql_for_views(&project, &fs, &database, false, false).unwrap();

        assert_eq!(sql, expected_output)
    }

    #[test]
    fn test_project_and_fs_to_sql_for_views_sqlite() {
        let database = DatabaseQueryGeneratorSqlite::default();
        let fs = quary_proto::FileSystem {
            files: HashMap::from([
                (
                    "models/stg_shifts.sql".to_string(),
                    quary_proto::File {
                        name: "models/stg_shifts.sql".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "SELECT employee_id, shift_date, shift FROM q.raw_shifts",
                        ),
                    },
                ),
                (
                    "models/shifts_transformed.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts_transformed.sql".to_string(),
                        contents: prost::bytes::Bytes::from("SELECT * FROM q.stg_shifts"),
                    },
                ),
                (
                    "models/schema.yaml".to_string(),
                    quary_proto::File {
                        name: "models/schema.yaml".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "sources: [{name: raw_shifts, path: raw_shifts_real_table}]",
                        ),
                    },
                ),
            ]),
        };

        let expected_output = vec![
            (
                "stg_shifts".to_string(), 
                vec![
                    "DROP VIEW IF EXISTS `stg_shifts`".to_string(), 
                    "CREATE VIEW `stg_shifts` AS SELECT employee_id, shift_date, shift FROM `raw_shifts_real_table`".to_string()
                ]
            ),
            (
                "shifts_transformed".to_string(), 
                vec![
                    "DROP VIEW IF EXISTS `shifts_transformed`".to_string(), 
                    "CREATE VIEW `shifts_transformed` AS SELECT * FROM `stg_shifts`".to_string()
                ]
            )
        ];

        let project = parse_project(&fs, &database, "").unwrap();
        let sql = project_and_fs_to_sql_for_views(&project, &fs, &database, false, false).unwrap();

        assert_eq!(sql, expected_output)
    }

    #[test]
    fn test_return_tests_sql_no_roots() {}

    #[test]
    fn test_parse_project_with_bad_reference_but_in_comment() {
        let database = DatabaseQueryGeneratorSqlite {};

        let file_system = quary_proto::FileSystem {
            files: vec![
                (
                "models/shifts.sql".to_string(),
                quary_proto::File {
                    name: "models/shifts.sql".to_string(),
                    contents: prost::bytes::Bytes::from(
                        "SELECT employee_id, shift_date, shift FROM q.raw_shifts -- q.raw_gibberish_in_comment",
                    ),
                },
                ),
                (
                "models/shifts_again.sql".to_string(),
                quary_proto::File {
                    name: "models/shifts_again.sql".to_string(),
                    contents: prost::bytes::Bytes::from(
                        "SELECT employee_id, shift_date, shift FROM q.shifts /* q.raw_gibberish_in_comment */",
                    ),
                },
                ),
                (
                "tests/shifts_again.sql".to_string(),
                quary_proto::File {
                    name: "models/shifts_again.sql".to_string(),
                    contents: prost::bytes::Bytes::from(
                        "SELECT employee_id, shift_date, shift FROM q.shifts /* q.raw_gibberish_in_comment */",
                    ),
                },
                ),
                (
                    "models/schema.yaml".to_string(),
                    quary_proto::File {
                        name: "models/schema.yaml".to_string(),
                        contents: prost::bytes::Bytes::from("sources:
- name: raw_shifts
  path: raw_shifts_real_table"),
                    },
                ),
            ]
            .into_iter()
            .collect(),
        };

        parse_project(&file_system, &database, "").unwrap();
    }

    #[test]
    fn test_parse_project_with_sql_beginning_with_q() {
        let database = DatabaseQueryGeneratorSqlite {};

        let file_system = quary_proto::FileSystem {
            files: vec![
                (
                    "models/shifts.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts.sql".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "SELECT employee_id AS quary_id, shift_date, shift FROM q.raw_shifts",
                        ),
                    },
                ),
                (
                    "models/shifts_again.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts_again.sql".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "SELECT employee_id AS quary_id, shift_date, shift FROM q.shifts",
                        ),
                    },
                ),
                (
                    "tests/shifts_again.sql".to_string(),
                    quary_proto::File {
                        name: "models/shifts_again.sql".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "SELECT employee_id AS quary_id, shift_date, shift FROM q.shifts",
                        ),
                    },
                ),
                (
                    "models/schema.yaml".to_string(),
                    quary_proto::File {
                        name: "models/schema.yaml".to_string(),
                        contents: prost::bytes::Bytes::from(
                            "sources:
- name: raw_shifts
  path: raw_shifts_real_table",
                        ),
                    },
                ),
            ]
            .into_iter()
            .collect(),
        };

        parse_project(&file_system, &database, "").unwrap();
    }

    // TODO Reinstate after making get_node_sorted completely deterministic
    // #[test]
    // fn test_project_and_fs_to_sql_for_views() {
    //     let assets = Asset {};
    //     let config = default_config();
    //     let project = parse_project(&config, &assets, "").unwrap();
    //
    //     let sql = project_and_fs_to_query_sql(&project, &assets, "stg_shifts").unwrap();
    //     assert_eq!(
    //         sql.0,
    //         "WITH raw_shifts AS (SELECT column1 AS employee_id,column2 AS shop_id,column3 AS date,column4 AS shift FROM (VALUES ('1','2','2023-01-01','morning'),('1','2','2023-01-02','morning'),('1','2','2023-01-03','morning'),('1','2','2023-01-04','morning'),('1','2','2023-01-05','morning'),('1','2','2023-01-06','morning'),('1','2','2023-01-07','morning'),('1','2','2023-01-08','morning'),('1','2','2023-01-09','morning'),('1','2','2023-01-10','morning'),('1','2','2023-01-11','morning'),('1','2','2023-01-12','morning'),('1','2','2023-01-13','morning'),('1','2','2023-01-13','afternoon'))) select\n  employee_id,\n  shop_id,\n  date as shift_date,\n  shift\nfrom\n  raw_shifts\n"
    //     );
    //
    //     let sql = project_and_fs_to_query_sql(&project, &assets, "shifts_summary").unwrap();
    //     assert_eq!(
    //         sql.0,
    //         "WITH\nraw_employees AS (SELECT column1 AS id,column2 AS first_name,column3 AS last_name FROM (VALUES ('1','John','Doe'),('2','Jane','Doe'),('3','Ashok','Kumar'),('4','Peter','Pan'),('5','Marie','Curie'))),\nraw_shifts AS (SELECT column1 AS employee_id,column2 AS shop_id,column3 AS date,column4 AS shift FROM (VALUES ('1','2','2023-01-01','morning'),('1','2','2023-01-02','morning'),('1','2','2023-01-03','morning'),('1','2','2023-01-04','morning'),('1','2','2023-01-05','morning'),('1','2','2023-01-06','morning'),('1','2','2023-01-07','morning'),('1','2','2023-01-08','morning'),('1','2','2023-01-09','morning'),('1','2','2023-01-10','morning'),('1','2','2023-01-11','morning'),('1','2','2023-01-12','morning'),('1','2','2023-01-13','morning'),('1','2','2023-01-13','afternoon'))),\nshift_hours AS (SELECT 'morning'  AS shift,\n       '08:00:00' AS start_time,\n       '12:00:00' AS end_time\nUNION ALL\nSELECT 'afternoon' AS shift,\n       '12:00:00'  AS start_time,\n       '16:00:00'  AS end_time),\nshift_first AS (WITH\n  min_shifts AS (\n    SELECT\n      employee_id,\n      MIN(shift_start) AS shift_start\n    FROM\n      shifts\n    GROUP BY\n      employee_id\n  )\nSELECT\n  x.employee_id AS employee_id,\n  x.shift_start AS shift_start,\n  x.shift_end AS shift_end\nFROM\n  shifts x\n  INNER JOIN min_shifts y ON y.employee_id = x.employee_id\n  AND y.shift_start = x.shift_start\nGROUP BY\n  x.employee_id,\n  x.shift_start\n),\nshift_last AS (WITH min_shifts AS (SELECT employee_id,\n                           max(shift_start) AS shift_start\n                    FROM shifts\n                    GROUP BY employee_id)\n\nSELECT x.employee_id AS employee_id,\n       x.shift_start AS shift_start,\n       x.shift_end AS shift_end\nFROM shifts x\nINNER JOIN min_shifts y\nON y.employee_id = x.employee_id AND y.shift_start = x.shift_start\nGROUP BY x.employee_id, x.shift_start),\nstg_employees AS (select\n  id as employee_id,\n  first_name,\n  last_name\nfrom\n  raw_employees\n),\nstg_shifts AS (select\n  employee_id,\n  shop_id,\n  date as shift_date,\n  shift\nfrom\n  raw_shifts\n),\nshifts AS (WITH shifts AS (SELECT employee_id,\n                       shift_date,\n                       shift\n                FROM stg_shifts\n                ),\n     shift_details AS (SELECT shift AS shift_name,\n                              start_time,\n                              end_time\n                       FROM shift_hours\n                       )\n\nSELECT s.employee_id AS employee_id,\n       s.shift AS shift,\n       datetime(s.shift_date, sd.start_time) AS shift_start,\n       datetime(s.shift_date, sd.end_time)   AS shift_end\nFROM shifts s\n         INNER JOIN shift_details sd\n                    ON s.shift = sd.shift_name\n)\nSELECT * FROM (WITH total_hours AS (\n    SELECT employee_id,\n           SUM(strftime('%s', shift_end) - strftime('%s', shift_start)) AS total_hours,\n           COUNT(*) AS total_shifts\n    FROM shifts\n    GROUP BY employee_id\n),\n\npercentage_morning_shifts AS (\n    SELECT employee_id,\n           SUM(CASE WHEN shift = 'morning' THEN 1 ELSE 0 END) AS total_morning_shifts,\n          COUNT(*) AS total_shifts\n    FROM shifts\n    GROUP BY employee_id\n)\n\nSELECT e.employee_id,\n       e.first_name,\n       e.last_name,\n       sf.shift_start AS first_shift,\n       sl.shift_start AS last_shift,\n       pms.total_morning_shifts / pms.total_shifts * 100 AS percentage_morning_shifts,\n       th.total_shifts,\n       th.total_hours\nFROM stg_employees e\nLEFT JOIN shift_first sf\n    ON e.employee_id = sf.employee_id\nLEFT JOIN shift_last sl\n    ON e.employee_id = sl.employee_id\nLEFT JOIN total_hours th\n    ON e.employee_id = th.employee_id\nLEFT JOIN percentage_morning_shifts pms\n    ON e.employee_id = pms.employee_id)"
    //     )
    // }

    // TODO Implement tests
    //func Test_parseColumnTests(t *testing.T) {
    //	t.Parallel()
    //
    //	tests := []struct {
    //		name         string
    //		pathMap      PathMap
    //		customTests  map[string]*servicev1.CustomTest
    //		projectFiles map[string]*servicev1.ProjectFile
    //		want         map[string]*servicev1.Test
    //		wantErr      assert.ErrorAssertionFunc
    //	}{
    //		// TODO: Add test cases. Unrecognized test type should return an error.
    //		// TODO: Add tests for wrong info in relationship
    //		{
    //			name:        "simple example for model just references",
    //			customTests: map[string]*servicev1.CustomTest{},
    //			pathMap:     PathMap{"example": "example", "users": "users", "users_other": "users_other_path"},
    //			projectFiles: map[string]*servicev1.ProjectFile{
    //				"example/example.sql": {
    //					Models: []*servicev1.ProjectFile_Model{
    //						{
    //							Name: "example",
    //							Columns: []*servicev1.ProjectFile_Column{
    //								{
    //									Name:        "id",
    //									Description: "test",
    //									Tests: []*servicev1.ProjectFile_Column_ColumnTest{
    //										{
    //											Type: "relationship",
    //											Info: map[string]string{
    //												"model":  "users",
    //												"column": "id",
    //											},
    //										},
    //										{
    //											Type: "relationship",
    //											Info: map[string]string{
    //												"model":  "users_other",
    //												"column": "id",
    //											},
    //										},
    //									},
    //								},
    //							},
    //						},
    //					},
    //				},
    //			},
    //			want: map[string]*servicev1.Test{
    //				"test_example_id_relationship_users_id": {
    //					TestType: &servicev1.Test_Relationship{
    //						Relationship: &servicev1.TestRelationship{
    //							SourceModel:  "example",
    //							SourcePath:   "example",
    //							SourceColumn: "id",
    //							TargetModel:  "users",
    //							TargetPath:   "users",
    //							TargetColumn: "id",
    //							FilePath:     "example/example.sql",
    //						},
    //					},
    //				},
    //				"test_example_id_relationship_users_other_id": {
    //					TestType: &servicev1.Test_Relationship{
    //						Relationship: &servicev1.TestRelationship{
    //							SourceModel:  "example",
    //							SourcePath:   "example",
    //							SourceColumn: "id",
    //							TargetModel:  "users_other",
    //							TargetPath:   "users_other_path",
    //							TargetColumn: "id",
    //							FilePath:     "example/example.sql",
    //						},
    //					},
    //				},
    //			},
    //			wantErr: assert.NoError,
    //		},
    //		{
    //			name: "simple example for model",
    //			customTests: map[string]*servicev1.CustomTest{
    //				"gte": {
    //					FilePath: "custom_tests/gte.sql",
    //					Name:     "gte",
    //					Sql:      "SELECT * FROM {{ .Model }} WHERE {{ .Column }} < {{ .Value }}",
    //				},
    //			},
    //			pathMap: PathMap{"example": "example", "users": "users", "users_other": "users_other_path"},
    //			projectFiles: map[string]*servicev1.ProjectFile{
    //				"example/example.sql": {
    //					Models: []*servicev1.ProjectFile_Model{
    //						{
    //							Name: "example",
    //							Columns: []*servicev1.ProjectFile_Column{
    //								{
    //									Name:        "id",
    //									Description: "test",
    //									Tests: []*servicev1.ProjectFile_Column_ColumnTest{
    //										{
    //											Type: "not_null",
    //										},
    //										{
    //											Type: "unique",
    //										},
    //										{
    //											Type: "relationship",
    //											Info: map[string]string{
    //												"model":  "users",
    //												"column": "id",
    //											},
    //										},
    //										{
    //											Type: "relationship",
    //											Info: map[string]string{
    //												"model":  "users_other",
    //												"column": "id",
    //											},
    //										},
    //										{
    //											Type: "accepted_values",
    //											Info: map[string]string{
    //												"values": "1,2,3",
    //											},
    //										},
    //										{
    //											Type: "gte",
    //											Info: map[string]string{
    //												"value": "0",
    //											},
    //										},
    //									},
    //								},
    //							},
    //						},
    //					},
    //				},
    //			},
    //			want: map[string]*servicev1.Test{
    //				"test_example_id_unique": {
    //					TestType: &servicev1.Test_Unique{
    //						Unique: &servicev1.TestUnique{
    //							Model:    "example",
    //							Path:     "example",
    //							Column:   "id",
    //							FilePath: "example/example.sql",
    //						},
    //					},
    //				},
    //				"test_example_id_not_null": {
    //					TestType: &servicev1.Test_NotNull{
    //						NotNull: &servicev1.TestNotNull{
    //							Model:    "example",
    //							Path:     "example",
    //							Column:   "id",
    //							FilePath: "example/example.sql",
    //						},
    //					},
    //				},
    //				"test_example_id_relationship_users_id": {
    //					TestType: &servicev1.Test_Relationship{
    //						Relationship: &servicev1.TestRelationship{
    //							SourceModel:  "example",
    //							SourcePath:   "example",
    //							SourceColumn: "id",
    //							TargetModel:  "users",
    //							TargetPath:   "users",
    //							TargetColumn: "id",
    //							FilePath:     "example/example.sql",
    //						},
    //					},
    //				},
    //				"test_example_id_relationship_users_other_id": {
    //					TestType: &servicev1.Test_Relationship{
    //						Relationship: &servicev1.TestRelationship{
    //							SourceModel:  "example",
    //							SourcePath:   "example",
    //							SourceColumn: "id",
    //							TargetModel:  "users_other",
    //							TargetPath:   "users_other_path",
    //							TargetColumn: "id",
    //							FilePath:     "example/example.sql",
    //						},
    //					},
    //				},
    //				"test_example_id_accepted_values": {
    //					TestType: &servicev1.Test_AcceptedValues{
    //						AcceptedValues: &servicev1.TestAcceptedValues{
    //							Model:          "example",
    //							Path:           "example",
    //							Column:         "id",
    //							FilePath:       "example/example.sql",
    //							AcceptedValues: []string{"1", "2", "3"},
    //						},
    //					},
    //				},
    //				"test_example_id_gte": {
    //					TestType: &servicev1.Test_CustomColumn{
    //						CustomColumn: &servicev1.TestCustomColumn{
    //							TestFilePath: "custom_tests/gte.sql",
    //							TestName:     "gte",
    //							OriginalSql:  "SELECT * FROM {{ .Model }} WHERE {{ .Column }} < {{ .Value }}",
    //							Model:        "example",
    //							Path:         "example",
    //							Column:       "id",
    //							Info: map[string]string{
    //								"value": "0",
    //							},
    //							RenderedSql: "SELECT * FROM example WHERE id < 0",
    //						},
    //					},
    //				},
    //			},
    //			wantErr: assert.NoError,
    //		},
    //		{
    //			name:    "simple example for source",
    //			pathMap: PathMap{"example": "example", "users": "users", "users_other": "users_other_path"},
    //			customTests: map[string]*servicev1.CustomTest{
    //				"gte": {
    //					FilePath: "custom_tests/gte.sql",
    //					Name:     "gte",
    //					Sql:      "SELECT * FROM {{ .Model }} WHERE {{ .Column }} < {{ .Value }}",
    //				},
    //			},
    //			projectFiles: map[string]*servicev1.ProjectFile{
    //				"example/example.sql": {
    //					Sources: []*servicev1.ProjectFile_Source{
    //						{
    //							Name: "example",
    //							Path: "example_123",
    //							Columns: []*servicev1.ProjectFile_Column{
    //								{
    //									Name:        "id",
    //									Description: "test",
    //									Tests: []*servicev1.ProjectFile_Column_ColumnTest{
    //										{
    //											Type: "not_null",
    //										},
    //										{
    //											Type: "unique",
    //										},
    //										{
    //											Type: "relationship",
    //											Info: map[string]string{
    //												"model":  "users",
    //												"column": "id",
    //											},
    //										},
    //										{
    //											Type: "relationship",
    //											Info: map[string]string{
    //												"model":  "users_other",
    //												"column": "id",
    //											},
    //										},
    //										{
    //											Type: "accepted_values",
    //											Info: map[string]string{
    //												"values": "1,2,3",
    //											},
    //										},
    //										{
    //											Type: "gte",
    //											Info: map[string]string{
    //												"value": "0",
    //											},
    //										},
    //									},
    //								},
    //							},
    //						},
    //					},
    //				},
    //			},
    //			want: map[string]*servicev1.Test{
    //				"test_example_id_unique": {
    //					TestType: &servicev1.Test_Unique{
    //						Unique: &servicev1.TestUnique{
    //							Model:    "example",
    //							Path:     "example_123",
    //							Column:   "id",
    //							FilePath: "example/example.sql",
    //						},
    //					},
    //				},
    //				"test_example_id_not_null": {
    //					TestType: &servicev1.Test_NotNull{
    //						NotNull: &servicev1.TestNotNull{
    //							Model:    "example",
    //							Path:     "example_123",
    //							Column:   "id",
    //							FilePath: "example/example.sql",
    //						},
    //					},
    //				},
    //				"test_example_id_relationship_users_id": {
    //					TestType: &servicev1.Test_Relationship{
    //						Relationship: &servicev1.TestRelationship{
    //							SourceModel:  "example",
    //							SourcePath:   "example_123",
    //							SourceColumn: "id",
    //							TargetModel:  "users",
    //							// TODO Show that this can be a path to a source as well
    //							TargetPath:   "users",
    //							TargetColumn: "id",
    //							FilePath:     "example/example.sql",
    //						},
    //					},
    //				},
    //				"test_example_id_relationship_users_other_id": {
    //					TestType: &servicev1.Test_Relationship{
    //						Relationship: &servicev1.TestRelationship{
    //							SourceModel:  "example",
    //							SourcePath:   "example_123",
    //							SourceColumn: "id",
    //							TargetModel:  "users_other",
    //							TargetPath:   "users_other_path",
    //							TargetColumn: "id",
    //							FilePath:     "example/example.sql",
    //						},
    //					},
    //				},
    //				"test_example_id_accepted_values": {
    //					TestType: &servicev1.Test_AcceptedValues{
    //						AcceptedValues: &servicev1.TestAcceptedValues{
    //							Model:          "example",
    //							Path:           "example_123",
    //							Column:         "id",
    //							FilePath:       "example/example.sql",
    //							AcceptedValues: []string{"1", "2", "3"},
    //						},
    //					},
    //				},
    //				"test_example_id_gte": {
    //					TestType: &servicev1.Test_CustomColumn{
    //						CustomColumn: &servicev1.TestCustomColumn{
    //							TestFilePath: "custom_tests/gte.sql",
    //							TestName:     "gte",
    //							OriginalSql:  "SELECT * FROM {{ .Model }} WHERE {{ .Column }} < {{ .Value }}",
    //							Model:        "example",
    //							Path:         "example_123",
    //							Column:       "id",
    //							Info: map[string]string{
    //								"value": "0",
    //							},
    //							RenderedSql: "SELECT * FROM example_123 WHERE id < 0",
    //						},
    //					},
    //				},
    //			},
    //			wantErr: assert.NoError,
    //		},
    //	}
    //	for _, tt := range tests {
    //		t.Run(tt.name, func(t *testing.T) {
    //			got, err := parseColumnTests(tt.customTests, tt.projectFiles, tt.pathMap)
    //
    //			if !tt.wantErr(t, err, fmt.Sprintf("parseColumnTests(%v)", tt.projectFiles)) {
    //				return
    //			}
    //			assert.Equalf(t, tt.want, got, "parseColumnTests(%v)", tt.projectFiles)
    //		})
    //	}
    //}
    //
    //func Test_parseModel(t *testing.T) {
    //	tests := []struct {
    //		name    string
    //		fsfs    fs.FS
    //		c       *servicev1.Configuration
    //		ms      ModelDefinitions
    //		path    string
    //		want    *servicev1.Model
    //		wantErr assert.ErrorAssertionFunc
    //	}{
    //		{
    //			name: "valid model",
    //			// TODO Should refactor this so that over time it's easier to build fresh model
    //			ms: ModelDefinitions{
    //				"example": {
    //					Name:        "example",
    //					Description: "This is an example model",
    //				},
    //			},
    //			fsfs: func() fs.FS {
    //				f, err := NewFileSystem(&servicev1.FileSystem{Files: map[string]*servicev1.File{
    //					"/models/example.sql": {
    //						Name: "/models/example.sql",
    //						Contents: []byte(`
    //WITH shifts AS (SELECT employee_id,
    //                       shift_date,
    //                       shift
    //                FROM q.stg_shifts
    //                ),
    //     shift_details AS (SELECT shift AS shift_name,
    //                              start_time,
    //                              end_time
    //                       FROM q.shift_hours
    //                       )
    //
    //SELECT s.employee_id AS employee_id,
    //       s.shift AS shift,
    //       datetime(s.shift_date, sd.start_time) AS shift_start,
    //       datetime(s.shift_date, sd.end_time)   AS shift_end
    //FROM shifts s
    //         INNER JOIN shift_details sd
    //                    ON s.shift = sd.shift_name
    //`),
    //					},
    //					"/models/example.yaml": {
    //						Name: "/models/example.yaml",
    //						Contents: []byte(`
    //models:
    //  - name: example
    //    description: description of the model
    //`),
    //					},
    //				}})
    //				require.NoError(t, err)
    //				return f
    //			}(),
    //			c: &servicev1.Configuration{
    //				SchemaName: proto.String("q"),
    //			},
    //			path:    "models/example.sql",
    //			wantErr: assert.NoError,
    //			want: &servicev1.Model{
    //				Name:        "example",
    //				Description: "This is an example model",
    //				References:  []string{"stg_shifts", "shift_hours"},
    //			},
    //		},
    //	}
    //	for _, tt := range tests {
    //		t.Run(tt.name, func(t *testing.T) {
    //			got, err := parseModel(tt.fsfs, tt.ms, tt.c, tt.path)
    //			if !tt.wantErr(t, err, fmt.Sprintf("parseModel(%v, %v, %v)", tt.fsfs, tt.c, tt.path)) {
    //				return
    //			}
    //
    //			assert.Equalf(t, tt.want.GetName(), got.GetName(), "parseModel(%v, %v, %v)", tt.fsfs, tt.c, tt.path)
    //			assert.Equalf(t, tt.want.GetReferences(), got.GetReferences(), "parseModel(%v, %v, %v)", tt.fsfs, tt.c, tt.path)
    //		})
    //	}
    //}
}
