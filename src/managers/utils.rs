use std::fmt;
use std::ops::Range;

pub(super) struct Spanned<T> {
    pub(super) value: T,
    pub(super) span: Range<usize>,
}

impl<T> fmt::Debug for Spanned<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{:?}@{:?}", self.value, self.span)
    }
}
