use serde_yaml::Value;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

/// This function reads YAML files from a specified folder path and returns a HashMap containing the
/// file names and their contents.
///
/// Arguments:
///
/// * `folder_path`: A string representing the path to the folder containing the YAML files to be read.
///
/// Returns:
///
/// The function `read_yaml_files` returns a `HashMap` containing the contents of all YAML files in the
/// specified folder, with the keys being the file names without extension.
pub fn read_yaml_files(folder_path: &str) -> HashMap<String, String> {
    let mut data_map: HashMap<String, String> = HashMap::new();

    if let Ok(entries) = fs::read_dir(folder_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let file_path = entry.path();
                if file_path.is_file() {
                    if let Some(extension) = file_path.extension() {
                        if extension == "yaml" || extension == "yml" {
                            let contents = read_file_contents(&file_path);

                            // Extract the file name without extension
                            let file_name = get_relative_file_name(&file_path, folder_path);

                            data_map.insert(file_name, contents);
                        }
                    }
                }
            }
        }
    }

    data_map
}

/// This function returns the relative file name of a file given its path and the path of its parent
/// folder.
///
/// Arguments:
///
/// * `file_path`: `file_path` is a reference to a `Path` object, which represents the path to a file.
/// * `folder_path`: The `folder_path` parameter is a string representing the path of a folder on the
/// file system.
///
/// Returns:
///
/// The function `get_relative_file_name` returns a `String` that represents the relative path of a file
/// with respect to a folder.
fn get_relative_file_name(file_path: &Path, folder_path: &str) -> String {
    let file_path = file_path.strip_prefix(folder_path).unwrap();
    file_path.to_string_lossy().to_string().replace('\\', "/")
}

/// This function reads the contents of a file and returns them as a string.
///
/// Arguments:
///
/// * `file_path`: A reference to a Path object that represents the path to the file that needs to be
/// read.
///
/// Returns:
///
/// The function `read_file_contents` returns a `String` that contains the contents of the file located
/// at the given `file_path`.
fn read_file_contents(file_path: &Path) -> String {
    let mut file = fs::File::open(file_path).expect("Failed to open file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Failed to read file");
    contents
}

/// The function merges two values, either as a mapping or a sequence, into a single value.
///
/// Arguments:
///
/// * `destination`: The destination parameter is a Value type, which can be either a Mapping or a
/// Sequence. It represents the value that will be updated with the values from the source parameter.
/// * `source`: The `source` parameter is a `Value` type, which can be either a `Mapping` or a
/// `Sequence` containing key-value pairs or a list of values respectively. It is the value that needs
/// to be merged into the `destination` value.
///
/// Returns:
///
/// a `Value` which is the result of merging the `destination` and `source` values. The returned value
/// can be one of the following:
/// - If both `destination` and `source` are mappings, a new mapping is returned with the merged
/// key-value pairs.
/// - If both `destination` and `source` are sequences, a new sequence is returned with the elements of
fn merge_values(destination: Value, source: Value) -> Value {
    match (destination, source) {
        (Value::Mapping(mut map1), Value::Mapping(map2)) => {
            for (key, value) in map2 {
                if let Some(existing_value) = map1.remove(&key) {
                    let merged_value = merge_values(existing_value, value);
                    map1.insert(key, merged_value);
                } else {
                    map1.insert(key, value);
                }
            }
            Value::Mapping(map1)
        }
        (Value::Sequence(mut seq1), Value::Sequence(seq2)) => {
            seq1.extend(seq2);
            Value::Sequence(seq1)
        }
        (_, value) => value,
    }
}

/// The function merges two YAML files and returns the merged result.
///
/// Arguments:
///
/// * `current_config`: A string representing the path to the current configuration file.
/// * `new_config`: The path to the file containing the new configuration data that needs to be merged
/// with the current configuration.
///
/// Returns:
///
/// a `Result` containing a `Value` or a `Box` that implements the `std::error::Error` trait.
pub fn merge(current_config: &str, new_config: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let destination_contents = match fs::read_to_string(current_config) {
        Ok(contents) => contents,
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                // Create an empty file
                let mut file = File::create(current_config)?;
                file.write_all(b"")?;
                String::new()
            } else {
                return Err(Box::new(err));
            }
        }
    };
    let merged_yaml = serde_yaml::from_str::<Value>(&destination_contents)?;
    let source_contents = fs::read_to_string(new_config)?;
    let source = serde_yaml::from_str::<Value>(&source_contents)?;

    // Merge the YAML values
    let merged_value = merge_values(merged_yaml, source);

    Ok(merged_value)
}
