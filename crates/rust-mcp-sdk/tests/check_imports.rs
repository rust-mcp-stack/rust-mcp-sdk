#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::{self, Read};
    use std::path::Path;

    // List of files to exclude from the check
    const EXCLUDED_FILES: &[&str] = &["src/schema.rs"];

    // Check all .rs files for incorrect `use rust_mcp_schema` imports
    #[test]
    fn check_no_rust_mcp_schema_imports() {
        let mut errors = Vec::new();

        // Walk through the src directory
        for entry in walk_src_dir("src").expect("Failed to read src directory") {
            let entry = entry.unwrap();
            let path = entry.path();

            // only check files with .rs extension
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("rs") {
                let abs_path = path.to_string_lossy();
                let relative_path = path.strip_prefix("src").unwrap_or(&path);
                let path_str = relative_path.to_string_lossy();

                // Skip excluded files
                if EXCLUDED_FILES.iter().any(|&excluded| abs_path == excluded) {
                    continue;
                }

                // Read the file content
                match read_file(&path) {
                    Ok(content) => {
                        // Check for `use rust_mcp_schema`
                        if content.contains("use rust_mcp_schema") {
                            errors.push(format!(
                                "File {} contains `use rust_mcp_schema`. Use `use crate::schema` instead.",
                                abs_path
                            ));
                        }
                    }
                    Err(e) => {
                        errors.push(format!("Failed to read file `{}`: {}", path_str, e));
                    }
                }
            }
        }

        // If there are any errors, fail the test with all error messages
        if !errors.is_empty() {
            panic!(
                "Found {} incorrect imports:\n{}\n\n",
                errors.len(),
                errors.join("\n")
            );
        }
    }

    // Helper function to walk the src directory
    fn walk_src_dir<P: AsRef<Path>>(
        path: P,
    ) -> io::Result<impl Iterator<Item = io::Result<std::fs::DirEntry>>> {
        Ok(std::fs::read_dir(path)?.flat_map(|entry| {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                // Recursively walk subdirectories
                walk_src_dir(&path)
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
            } else {
                vec![Ok(entry)]
            }
        }))
    }

    // Helper function to read file content
    fn read_file(path: &Path) -> io::Result<String> {
        let mut file = File::open(path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        Ok(content)
    }
}
