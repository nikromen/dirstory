use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub struct Stack {
    path: PathBuf,
}

impl Stack {
    pub fn new(path: &str) -> Self {
        Stack {
            path: PathBuf::from(path),
        }
    }

    pub fn push(&mut self, item: &str) {
        let mut file = OpenOptions::new().append(true).open(&self.path).unwrap();

        writeln!(file, "{}", item).unwrap();
    }

    pub fn pop(&mut self, n: usize) -> Vec<String> {
        let (result, position) = self.read_n_with_position(n);
        let file = OpenOptions::new().write(true).open(&self.path).unwrap();

        if position > 0 {
            // include the newline character
            file.set_len(position).unwrap();
        } else {
            file.set_len(0).unwrap();
        }

        result
    }

    pub fn get_n(&mut self, n: usize) -> Vec<String> {
        let (result, _) = self.read_n_with_position(n);
        result
    }

    pub fn empty(&self) -> io::Result<()> {
        let file = OpenOptions::new().write(true).open(&self.path).unwrap();

        file.set_len(0).unwrap();
        Ok(())
    }

    fn read_n_with_position(&mut self, n: usize) -> (Vec<String>, u64) {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&self.path)
            .unwrap();

        let mut reader = BufReader::new(file);
        let end = reader.seek(SeekFrom::End(0)).unwrap();
        let mut position = end;

        let mut current_line = Vec::new();
        let mut result = Vec::with_capacity(n);

        while result.len() < n && position > 0 {
            position -= 1;
            reader.seek(SeekFrom::Start(position)).unwrap();

            let mut byte_buffer = [0u8; 1];
            reader.read_exact(&mut byte_buffer).unwrap();

            if byte_buffer[0] != b'\n' {
                current_line.push(byte_buffer[0]);
            }

            if byte_buffer[0] == b'\n' && !current_line.is_empty() || position == 0 {
                current_line.reverse();
                result.push(String::from_utf8(current_line.clone()).unwrap());
                current_line.clear();
                continue;
            }
        }

        if position > 0 && position < end {
            // include the newline character
            position += 1;
        }
        (result, position)
    }
}

pub fn get_or_create_stack_from_path(path: &str) -> Stack {
    let file_path = Path::new(path);
    if !file_path.exists() {
        let parent = file_path.parent().unwrap();
        if !parent.exists() {
            std::fs::create_dir_all(parent).unwrap();
        }

        File::create(file_path).unwrap();
    }

    Stack::new(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::{tempdir, NamedTempFile};
    use test_case::test_case;
    use uuid::Uuid;

    #[test_case("a\nb\nc\nd\n", 2, vec!["d", "c"], "a\nb\n" ; "pop 2")]
    #[test_case("a\nb\nc\nd\n", 4, vec!["d", "c", "b", "a"], "" ; "pop all")]
    #[test_case("a\nb\nc\nd\n", 0, vec![], "a\nb\nc\nd\n" ; "pop none")]
    #[test_case("a\nb\nc\nd\n", 1, vec!["d"], "a\nb\nc\n" ; "pop 1")]
    #[test_case("a\nb\nc\n", 6, vec!["c", "b", "a"], "" ; "pop more than available")]
    fn test_pop(content: &str, n: usize, expected: Vec<&str>, expected_file_content: &str) {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();

        file.write_all(content.as_bytes()).unwrap();

        let mut stack = Stack::new(&path);
        let result = stack.pop(n);

        assert_eq!(result, expected);

        let actual_file_content = fs::read_to_string(&path).unwrap();
        assert_eq!(actual_file_content, expected_file_content);
    }

    #[test_case("a\nb\nc\nd\n", 2, vec!["d", "c"], "a\nb\nc\nd\n" ; "get 2")]
    #[test_case("a\nb\nc\nd\n", 4, vec!["d", "c", "b", "a"], "a\nb\nc\nd\n" ; "get all")]
    #[test_case("a\nb\nc\nd\n", 0, vec![], "a\nb\nc\nd\n" ; "get none")]
    #[test_case("a\nb\nc\n", 1, vec!["c"], "a\nb\nc\n" ; "get 1")]
    #[test_case("a\nb\nc\n", 6, vec!["c", "b", "a"], "a\nb\nc\n" ; "get more than available")]
    fn test_get_n(content: &str, n: usize, expected: Vec<&str>, expected_file_content: &str) {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();

        file.write_all(content.as_bytes()).unwrap();

        let mut stack = Stack::new(&path);
        let result = stack.get_n(n);

        assert_eq!(result, expected);

        let actual_file_content = fs::read_to_string(&path).unwrap();
        assert_eq!(actual_file_content, expected_file_content);
    }

    #[test_case("a\nb\nc\nd\n", "" ; "empty non-empty")]
    #[test_case("", "" ; "empty empty")]
    fn test_empty(content: &str, expected_file_content: &str) {
        let mut file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();

        file.write_all(content.as_bytes()).unwrap();

        let stack = Stack::new(&path);
        stack.empty().unwrap();

        let actual_file_content = fs::read_to_string(&path).unwrap();
        assert_eq!(actual_file_content, expected_file_content);
    }

    #[test]
    fn test_create_stack_from_path() {
        let dir = tempdir().unwrap();
        let random_file_name = Uuid::new_v4().to_string();
        let path = dir.path().join(random_file_name);

        let stack = get_or_create_stack_from_path(path.to_str().unwrap());
        assert!(path.exists());
        assert_eq!(stack.path, path);
    }

    #[test]
    fn test_create_stack_from_path_with_parent() {
        let dir = tempdir().unwrap();
        let random_file_name = Uuid::new_v4().to_string();
        let path = dir.path().join("subdir").join(random_file_name);

        let stack = get_or_create_stack_from_path(path.to_str().unwrap());

        assert!(stack.path.parent().unwrap().exists());
        assert_eq!(stack.path.parent().unwrap(), dir.path().join("subdir"));

        assert!(path.exists());
        assert_eq!(stack.path, path);
    }

    #[test]
    fn test_get_stack_from_path() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let stack = get_or_create_stack_from_path(&path);
        assert!(Path::new(&path).exists());
        assert_eq!(stack.path, PathBuf::from(path));
    }

    #[test]
    fn test_push() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap().to_string();

        let mut stack = Stack::new(&path);

        let mut actual_file_content = fs::read_to_string(&path).unwrap();
        assert_eq!(actual_file_content, "");

        stack.push("a");
        actual_file_content = fs::read_to_string(&path).unwrap();
        assert_eq!(actual_file_content, "a\n");

        stack.push("b");
        actual_file_content = fs::read_to_string(&path).unwrap();
        assert_eq!(actual_file_content, "a\nb\n");
    }
}
