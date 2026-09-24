//! Workflow execution context with heterogeneous type storage.

use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;
use std::time::Instant;

/// String key under which a value is stored in a [`Context`].
///
/// A `ContextKey` does not constrain the value type. Use [`Key`] for keys
/// that are checked against the value type at compile time.
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

/// A context key bound to the type of the value it stores.
///
/// Declaring keys as constants lets the compiler check that a key is always
/// used with the same value type, and removes the need for type annotations
/// when reading values back.
///
/// Typed keys share the namespace of plain string keys: `Key::<u64>::new("id")`
/// and `"id"` address the same entry.
///
/// # Examples
///
/// ```
/// use tsumugi_core::{Context, Key};
///
/// const USER_ID: Key<u64> = Key::new("user_id");
///
/// let mut ctx = Context::new();
/// ctx.insert(USER_ID, 42);
///
/// let id: Option<&u64> = ctx.get(USER_ID); // type inferred from the key
/// assert_eq!(id, Some(&42));
/// ```
///
/// Using a key with the wrong value type fails to compile:
///
/// ```compile_fail
/// use tsumugi_core::{Context, Key};
///
/// const USER_ID: Key<u64> = Key::new("user_id");
///
/// let mut ctx = Context::new();
/// ctx.insert(USER_ID, "not a number");
/// ```
pub struct Key<T> {
    name: &'static str,
    // `fn() -> T` keeps `Key<T>` `Send + Sync + Copy` regardless of `T`.
    _marker: PhantomData<fn() -> T>,
}

impl<T> Key<T> {
    /// Creates a new typed key.
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            _marker: PhantomData,
        }
    }

    /// Returns the key name.
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

impl<T> Clone for Key<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Key<T> {}

impl<T> fmt::Debug for Key<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Key")
            .field(&self.name)
            .field(&std::any::type_name::<T>())
            .finish()
    }
}

impl<T> AsRef<str> for Key<T> {
    fn as_ref(&self) -> &str {
        self.name
    }
}

impl<T> From<Key<T>> for ContextKey {
    fn from(key: Key<T>) -> Self {
        Self::new(key.name)
    }
}

/// Types that can address a value of type `T` in a [`Context`].
///
/// Plain string keys (`&str`, `String`, [`ContextKey`]) work with any value
/// type, while a typed [`Key<T>`] only works with `T`.
pub trait KeyFor<T> {
    /// Returns the key name.
    fn key_name(&self) -> &str;
}

impl<T> KeyFor<T> for Key<T> {
    fn key_name(&self) -> &str {
        self.name
    }
}

impl<T> KeyFor<T> for &str {
    fn key_name(&self) -> &str {
        self
    }
}

impl<T> KeyFor<T> for String {
    fn key_name(&self) -> &str {
        self
    }
}

impl<T> KeyFor<T> for &String {
    fn key_name(&self) -> &str {
        self
    }
}

impl<T> KeyFor<T> for ContextKey {
    fn key_name(&self) -> &str {
        &self.0
    }
}

impl<T> KeyFor<T> for &ContextKey {
    fn key_name(&self) -> &str {
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
    pub fn insert<T: Any + Send + Sync>(&mut self, key: impl KeyFor<T>, value: T) {
        self.data
            .insert(ContextKey::new(key.key_name()), Box::new(value));
    }

    /// Returns a reference to the value for the given key.
    ///
    /// Returns `None` if the key doesn't exist or the type doesn't match.
    pub fn get<T: Any>(&self, key: impl KeyFor<T>) -> Option<&T> {
        self.data
            .get(key.key_name())
            .and_then(|v| v.downcast_ref::<T>())
    }

    /// Returns a mutable reference to the value for the given key.
    ///
    /// Returns `None` if the key doesn't exist or the type doesn't match.
    pub fn get_mut<T: Any>(&mut self, key: impl KeyFor<T>) -> Option<&mut T> {
        self.data
            .get_mut(key.key_name())
            .and_then(|v| v.downcast_mut::<T>())
    }

    /// Removes a value by key and returns it.
    ///
    /// Returns `None` if the key doesn't exist or the type doesn't match.
    /// On a type mismatch the entry is left in place.
    pub fn remove<T: Any>(&mut self, key: impl KeyFor<T>) -> Option<T> {
        let name = key.key_name();
        if !self.data.get(name)?.is::<T>() {
            return None;
        }
        self.data
            .remove(name)
            .and_then(|v| v.downcast::<T>().ok())
            .map(|b| *b)
    }

    /// Returns `true` if the context contains a value for the given key.
    pub fn contains_key(&self, key: impl AsRef<str>) -> bool {
        self.data.contains_key(key.as_ref())
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
    fn test_remove_wrong_type_keeps_entry() {
        let mut ctx = Context::new();
        ctx.insert("key", 1i32);

        assert_eq!(ctx.remove::<String>("key"), None);
        assert_eq!(ctx.get::<i32>("key"), Some(&1));
    }

    #[test]
    fn test_typed_key() {
        const COUNT: Key<u32> = Key::new("count");
        let mut ctx = Context::new();

        ctx.insert(COUNT, 1);
        if let Some(count) = ctx.get_mut(COUNT) {
            *count += 1;
        }

        assert_eq!(ctx.get(COUNT), Some(&2));
        assert!(ctx.contains_key(COUNT));
        // Typed and string keys share the same namespace.
        assert_eq!(ctx.get::<u32>("count"), Some(&2));
        assert_eq!(ctx.remove(COUNT), Some(2));
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_context_key() {
        let key1 = ContextKey::new("test");
        let key2: ContextKey = "test".into();
        assert_eq!(key1, key2);
    }
}
