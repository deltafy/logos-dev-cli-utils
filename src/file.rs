use std::fs::{self, File};

pub fn read_file(path: &str) -> napi::Result<File> {
    fs::File::open(path)
        .map_err(|error| napi::Error::new(
                napi::Status::GenericFailure, 
                format!("Failed to open {}: {}", path, error)
        ))
}

pub fn create_file(path: &str) -> napi::Result<File> {
    fs::File::create(path)
        .map_err(|error| napi::Error::new(
            napi::Status::GenericFailure,
            format!("Failed to create {}: {}", path, error)
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::Path;

    fn setup_file(path: &str, contents: &str) {
        let mut file = fs::File::create(path).expect("Failed to create test file");
        file.write_all(contents.as_bytes()).expect("Failed to write to test file");
    }

    fn cleanup_file(path: &str) {
        if Path::new(path).exists() {
            fs::remove_file(path).expect("Failed to remove test file");
        }
    }

    #[test]
    fn test_read_file_success() {
        let filename = "test_read_success.txt";
        let data = "Test";
        setup_file(filename, data);

        let result = read_file(filename);
        assert!(result.is_ok(), "Successful file read expected");

        let file = result.unwrap();
        assert_eq!(file.metadata().unwrap().len(), 4, "File size should match");

        cleanup_file(filename);
    }

    #[test]
    fn test_read_file_failed() {
        let filename = "test_read_failed.txt";
        let result = read_file(filename);
        
        assert!(result.is_err(), "Failed file read expected");
    }

    #[test]
    fn test_create_file_success() {
        let filename = "test_create_success.txt";
        let result = create_file(filename);
        
        assert!(result.is_ok(), "Successful file creation expected");

        let file = result.unwrap();
        assert!(file.metadata().is_ok(), "File should exist");

        cleanup_file(filename);
    }

    #[test]
    fn test_create_file_failed() {
        let filename = "some_dir/test_create_failed.txt";
        let result = create_file(filename);

        assert!(result.is_err(), "Failed file read to nonexistent subdirectory expected");
    }
}
