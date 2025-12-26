//! Workflow execution context with heterogeneous type storage.

use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::time::Instant;

/// Type-safe context key wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContextKey(String);

impl ContextKey {
    /// Creates a new ContextKey.
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Returns the key as a string slice.
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

/// Execution context for workflow steps with heterogeneous type storage.
///
/// Stores any `Send + Sync` type, retrieved by downcasting.
///
/// # Examples
///
/// ```
/// use tsumugi_core::Context;
///
/// let mut ctx = Context::new();
///
/// // Store different types
/// ctx.insert("user_id", 123u64);
/// ctx.insert("name", "Alice".to_string());
/// ctx.insert("active", true);
///
/// // Retrieve with type annotation
/// assert_eq!(ctx.get::<u64>("user_id"), Some(&123));
/// assert_eq!(ctx.get::<String>("name"), Some(&"Alice".to_string()));
/// assert_eq!(ctx.get::<bool>("active"), Some(&true));
///
/// // Wrong type returns None
/// assert_eq!(ctx.get::<String>("user_id"), None);
/// ```
pub struct Context {
    data: HashMap<ContextKey, Box<dyn Any + Send + Sync>>,
    started_at: Instant,
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("keys", &self.data.keys().collect::<Vec<_>>())
            .field("started_at", &self.started_at)
            .finish()
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context {
    /// Creates a new empty context.
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            started_at: Instant::now(),
        }
    }

    /// Inserts a value with the given key.
    ///
    /// If the key already exists, the previous value is replaced.
    pub fn insert<T: Any + Send + Sync>(&mut self, key: impl Into<ContextKey>, value: T) {
        self.data.insert(key.into(), Box::new(value));
    }

    /// Returns a reference to the value for the given key.
    ///
    /// Returns `None` if the key doesn't exist or the type doesn't match.
    pub fn get<T: Any>(&self, key: &str) -> Option<&T> {
        self.data.get(key).and_then(|v| v.downcast_ref::<T>())
    }

    /// Returns a mutable reference to the value for the given key.
    ///
    /// Returns `None` if the key doesn't exist or the type doesn't match.
    pub fn get_mut<T: Any>(&mut self, key: &str) -> Option<&mut T> {
        self.data.get_mut(key).and_then(|v| v.downcast_mut::<T>())
    }

    /// Removes a value by key and returns it.
    ///
    /// Returns `None` if the key doesn't exist or the type doesn't match.
    pub fn remove<T: Any>(&mut self, key: &str) -> Option<T> {
        self.data
            .remove(key)
            .and_then(|v| v.downcast::<T>().ok())
            .map(|b| *b)
    }

    /// Returns `true` if the context contains a value for the given key.
    pub fn contains_key(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Returns an iterator over all keys in the context.
    pub fn keys(&self) -> impl Iterator<Item = &ContextKey> {
        self.data.keys()
    }

    /// Returns the number of entries in the context.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Returns `true` if the context contains no entries.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Removes all entries from the context.
    pub fn clear(&mut self) {
        self.data.clear();
    }

    /// Returns the time elapsed since the context was created.
    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heterogeneous_storage() {
        let mut ctx = Context::new();

        ctx.insert("int", 42i32);
        ctx.insert("string", "hello".to_string());
        ctx.insert("bool", true);

        assert_eq!(ctx.get::<i32>("int"), Some(&42));
        assert_eq!(ctx.get::<String>("string"), Some(&"hello".to_string()));
        assert_eq!(ctx.get::<bool>("bool"), Some(&true));

        // Wrong type returns None
        assert_eq!(ctx.get::<String>("int"), None);
    }

    #[test]
    fn test_get_mut() {
        let mut ctx = Context::new();
        ctx.insert("count", 0i32);

        if let Some(count) = ctx.get_mut::<i32>("count") {
            *count += 1;
        }

        assert_eq!(ctx.get::<i32>("count"), Some(&1));
    }

    #[test]
    fn test_remove() {
        let mut ctx = Context::new();
        ctx.insert("key", "value".to_string());

        let removed = ctx.remove::<String>("key");
        assert_eq!(removed, Some("value".to_string()));
        assert!(!ctx.contains_key("key"));
    }

    #[test]
    fn test_context_key() {
        let key1 = ContextKey::new("test");
        let key2: ContextKey = "test".into();
        assert_eq!(key1, key2);
    }
}
