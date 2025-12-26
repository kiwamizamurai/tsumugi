use std::collections::HashMap;
use std::fmt;
use std::time::Instant;

/// Type-safe context key wrapper.
///
/// Provides compile-time safety for context keys, preventing
/// typos and mismatched keys at the API level.
///
/// # Examples
///
/// ```
/// use tsumugi::ContextKey;
///
/// let key = ContextKey::new("user_id");
/// assert_eq!(key.as_str(), "user_id");
///
/// // Also works with From trait
/// let key: ContextKey = "session".into();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContextKey(String);

impl ContextKey {
    /// Creates a new ContextKey
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Returns the key as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContextKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ContextKey {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ContextKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl AsRef<str> for ContextKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for ContextKey {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// Execution context for workflow steps.
///
/// `Context` provides type-safe storage for sharing data between steps
/// during workflow execution. It tracks execution time and supports
/// metadata for debugging and monitoring.
///
/// # Type Parameter
///
/// * `T` - The type of values stored in the context
///
/// # Examples
///
/// ```
/// use tsumugi::Context;
///
/// let mut ctx = Context::<String>::new();
///
/// // Store and retrieve data
/// ctx.insert("user_id", "12345".to_string());
/// assert_eq!(ctx.get("user_id"), Some(&"12345".to_string()));
///
/// // Check existence
/// assert!(ctx.contains_key("user_id"));
/// assert!(!ctx.contains_key("nonexistent"));
///
/// // Remove data
/// let removed = ctx.remove("user_id");
/// assert_eq!(removed, Some("12345".to_string()));
/// ```
#[derive(Debug)]
pub struct Context<T> {
    data: HashMap<ContextKey, T>,
    start_time: Instant,
    metadata: HashMap<String, String>,
}

impl<T> Default for Context<T> {
    fn default() -> Self {
        Self {
            data: HashMap::new(),
            start_time: Instant::now(),
            metadata: HashMap::new(),
        }
    }
}

impl<T> Context<T> {
    /// Creates a new empty context.
    ///
    /// The context's timer starts when it is created.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::Context;
    ///
    /// let ctx = Context::<String>::new();
    /// assert!(ctx.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            start_time: Instant::now(),
            metadata: HashMap::new(),
        }
    }

    /// Inserts a value with the given key.
    ///
    /// If the key already exists, the previous value is replaced.
    ///
    /// # Arguments
    ///
    /// * `key` - Any type that can be converted into a `ContextKey` (e.g., `&str`, `String`)
    /// * `value` - The value to store
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::Context;
    ///
    /// let mut ctx = Context::<i32>::new();
    /// ctx.insert("count", 42);
    /// ctx.insert("count", 100); // Replaces previous value
    /// assert_eq!(ctx.get("count"), Some(&100));
    /// ```
    pub fn insert(&mut self, key: impl Into<ContextKey>, value: T) {
        self.data.insert(key.into(), value);
    }

    /// Returns a reference to the value for the given key.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to look up
    ///
    /// # Returns
    ///
    /// `Some(&T)` if the key exists, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::Context;
    ///
    /// let mut ctx = Context::<String>::new();
    /// ctx.insert("name", "Alice".to_string());
    ///
    /// assert_eq!(ctx.get("name"), Some(&"Alice".to_string()));
    /// assert_eq!(ctx.get("missing"), None);
    /// ```
    pub fn get(&self, key: &str) -> Option<&T> {
        self.data.get(key)
    }

    /// Returns a mutable reference to the value for the given key.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to look up
    ///
    /// # Returns
    ///
    /// `Some(&mut T)` if the key exists, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::Context;
    ///
    /// let mut ctx = Context::<i32>::new();
    /// ctx.insert("count", 0);
    ///
    /// if let Some(count) = ctx.get_mut("count") {
    ///     *count += 1;
    /// }
    /// assert_eq!(ctx.get("count"), Some(&1));
    /// ```
    pub fn get_mut(&mut self, key: &str) -> Option<&mut T> {
        self.data.get_mut(key)
    }

    /// Returns `true` if the context contains a value for the given key.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::Context;
    ///
    /// let mut ctx = Context::<String>::new();
    /// ctx.insert("key", "value".to_string());
    ///
    /// assert!(ctx.contains_key("key"));
    /// assert!(!ctx.contains_key("other"));
    /// ```
    pub fn contains_key(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Removes a value by key and returns it.
    ///
    /// # Returns
    ///
    /// `Some(T)` if the key existed, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::Context;
    ///
    /// let mut ctx = Context::<String>::new();
    /// ctx.insert("temp", "data".to_string());
    ///
    /// let removed = ctx.remove("temp");
    /// assert_eq!(removed, Some("data".to_string()));
    /// assert!(!ctx.contains_key("temp"));
    /// ```
    pub fn remove(&mut self, key: &str) -> Option<T> {
        self.data.remove(key)
    }

    /// Returns an iterator over all keys in the context.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::Context;
    ///
    /// let mut ctx = Context::<i32>::new();
    /// ctx.insert("a", 1);
    /// ctx.insert("b", 2);
    ///
    /// let keys: Vec<_> = ctx.keys().collect();
    /// assert_eq!(keys.len(), 2);
    /// ```
    pub fn keys(&self) -> impl Iterator<Item = &ContextKey> {
        self.data.keys()
    }

    /// Returns the number of entries in the context.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::Context;
    ///
    /// let mut ctx = Context::<String>::new();
    /// assert_eq!(ctx.len(), 0);
    ///
    /// ctx.insert("key", "value".to_string());
    /// assert_eq!(ctx.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the context contains no entries.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::Context;
    ///
    /// let mut ctx = Context::<String>::new();
    /// assert!(ctx.is_empty());
    ///
    /// ctx.insert("key", "value".to_string());
    /// assert!(!ctx.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Removes all entries from the context.
    ///
    /// This does not reset the elapsed time or metadata.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::Context;
    ///
    /// let mut ctx = Context::<String>::new();
    /// ctx.insert("a", "1".to_string());
    /// ctx.insert("b", "2".to_string());
    ///
    /// ctx.clear();
    /// assert!(ctx.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Sets a metadata value.
    ///
    /// Metadata is separate from the main data store and can be used
    /// for debugging, logging, or tracking workflow state.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::Context;
    ///
    /// let mut ctx = Context::<String>::new();
    /// ctx.set_metadata("workflow_id", "wf-123".to_string());
    /// ```
    pub fn set_metadata(&mut self, key: &str, value: String) {
        self.metadata.insert(key.to_string(), value);
    }

    /// Returns a metadata value by key.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::Context;
    ///
    /// let mut ctx = Context::<String>::new();
    /// ctx.set_metadata("version", "1.0".to_string());
    ///
    /// assert_eq!(ctx.get_metadata("version"), Some(&"1.0".to_string()));
    /// ```
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Returns the time elapsed since the context was created.
    ///
    /// Useful for tracking workflow execution time.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::Context;
    /// use std::time::Duration;
    ///
    /// let ctx = Context::<String>::new();
    /// std::thread::sleep(Duration::from_millis(10));
    /// assert!(ctx.elapsed() >= Duration::from_millis(10));
    /// ```
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

    #[test]
    fn test_context_key_creation() {
        let key1 = ContextKey::new("test");
        let key2 = ContextKey::from("test");
        let key3: ContextKey = "test".into();

        assert_eq!(key1, key2);
        assert_eq!(key2, key3);
        assert_eq!(key1.as_str(), "test");
    }

    #[test]
    fn test_context_contains_and_remove() {
        let mut ctx = Context::<String>::new();
        ctx.insert("key1", "value1".to_string());

        assert!(ctx.contains_key("key1"));
        assert!(!ctx.contains_key("key2"));

        let removed = ctx.remove("key1");
        assert_eq!(removed, Some("value1".to_string()));
        assert!(!ctx.contains_key("key1"));
    }

    #[test]
    fn test_context_keys_iterator() {
        let mut ctx = Context::<String>::new();
        ctx.insert("key1", "value1".to_string());
        ctx.insert("key2", "value2".to_string());

        let keys: Vec<&str> = ctx.keys().map(|k| k.as_str()).collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"key1"));
        assert!(keys.contains(&"key2"));
    }
}
