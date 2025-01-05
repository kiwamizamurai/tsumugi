use crate::error::WorkflowError;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug)]
pub struct Context<T> {
    data: HashMap<String, T>,
    errors: Vec<WorkflowError>,
    start_time: Instant,
    metadata: HashMap<String, String>,
}

impl<T> Default for Context<T> {
    fn default() -> Self {
        Self {
            data: HashMap::new(),
            errors: Vec::new(),
            start_time: Instant::now(),
            metadata: HashMap::new(),
        }
    }
}

impl<T> Context<T> {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            errors: Vec::new(),
            start_time: Instant::now(),
            metadata: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: &str, value: T) {
        self.data.insert(key.to_string(), value);
    }

    pub fn get(&self, key: &str) -> Option<&T> {
        self.data.get(key)
    }

    pub fn set_metadata(&mut self, key: &str, value: String) {
        self.metadata.insert(key.to_string(), value);
    }

    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_context_data_operations() {
        let mut ctx = Context::<String>::new();

        // データの挿入と取得をテスト
        ctx.insert("key1", "value1".to_string());
        assert_eq!(ctx.get("key1").map(|s| s.as_str()), Some("value1"));
        assert_eq!(ctx.get("nonexistent"), None);
    }

    #[test]
    fn test_context_metadata_operations() {
        let mut ctx = Context::<String>::new();

        // メタデータの操作をテスト
        ctx.set_metadata("meta1", "metadata1".to_string());
        assert_eq!(
            ctx.get_metadata("meta1").map(|s| s.as_str()),
            Some("metadata1")
        );
        assert_eq!(ctx.get_metadata("nonexistent"), None);
    }

    #[test]
    fn test_context_elapsed_time() {
        let ctx = Context::<String>::new();
        std::thread::sleep(Duration::from_millis(10));
        assert!(ctx.elapsed() >= Duration::from_millis(10));
    }
}
