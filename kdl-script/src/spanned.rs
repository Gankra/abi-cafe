use std::{
    borrow::Borrow,
    cmp::Ordering,
    fmt::{self, Display},
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
};

use miette::SourceSpan;
use serde::{ser, Deserialize};

/// A spanned value, indicating the range at which it is defined in the source.
#[derive(Clone, Default, Deserialize)]
pub struct Spanned<T> {
    start: usize,
    end: usize,
    value: T,
}

impl<T> Spanned<T> {
    pub fn new(value: T, span: SourceSpan) -> Self {
        Self {
            value,
            start: span.offset(),
            end: span.offset() + span.len(),
        }
    }
    /// Access the start of the span of the contained value.
    pub fn start(this: &Self) -> usize {
        this.start
    }

    /// Access the end of the span of the contained value.
    pub fn end(this: &Self) -> usize {
        this.end
    }

    /// Update the span
    pub fn update_span(this: &mut Self, start: usize, end: usize) {
        this.start = start;
        this.end = end;
    }

    /// Get the span of the contained value.
    pub fn span(this: &Self) -> SourceSpan {
        (Self::start(this)..Self::end(this)).into()
    }

    /// Consumes the spanned value and returns the contained value.
    pub fn into_inner(this: Self) -> T {
        this.value
    }
}

impl<T> IntoIterator for Spanned<T>
where
    T: IntoIterator,
{
    type IntoIter = T::IntoIter;
    type Item = T::Item;
    fn into_iter(self) -> Self::IntoIter {
        self.value.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a Spanned<T>
where
    &'a T: IntoIterator,
{
    type IntoIter = <&'a T as IntoIterator>::IntoIter;
    type Item = <&'a T as IntoIterator>::Item;
    fn into_iter(self) -> Self::IntoIter {
        self.value.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a mut Spanned<T>
where
    &'a mut T: IntoIterator,
{
    type IntoIter = <&'a mut T as IntoIterator>::IntoIter;
    type Item = <&'a mut T as IntoIterator>::Item;
    fn into_iter(self) -> Self::IntoIter {
        self.value.into_iter()
    }
}

impl<T> fmt::Debug for Spanned<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

impl<T> Display for Spanned<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

impl<T> Deref for Spanned<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for Spanned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl Borrow<str> for Spanned<String> {
    fn borrow(&self) -> &str {
        self
    }
}

impl<T, U: ?Sized> AsRef<U> for Spanned<T>
where
    T: AsRef<U>,
{
    fn as_ref(&self) -> &U {
        self.value.as_ref()
    }
}

impl<T: PartialEq> PartialEq for Spanned<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value.eq(&other.value)
    }
}

impl<T: PartialEq<T>> PartialEq<T> for Spanned<T> {
    fn eq(&self, other: &T) -> bool {
        self.value.eq(other)
    }
}

impl PartialEq<str> for Spanned<String> {
    fn eq(&self, other: &str) -> bool {
        self.value.eq(other)
    }
}

impl<T: Eq> Eq for Spanned<T> {}

impl<T: Hash> Hash for Spanned<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<T: PartialOrd> PartialOrd for Spanned<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl<T: PartialOrd<T>> PartialOrd<T> for Spanned<T> {
    fn partial_cmp(&self, other: &T) -> Option<Ordering> {
        self.value.partial_cmp(other)
    }
}

impl<T: Ord> Ord for Spanned<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl<T> From<T> for Spanned<T> {
    fn from(value: T) -> Self {
        Self {
            start: 0,
            end: 0,
            value,
        }
    }
}

impl<T: ser::Serialize> ser::Serialize for Spanned<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.value.serialize(serializer)
    }
}
