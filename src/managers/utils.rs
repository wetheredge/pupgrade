use std::cmp::Ordering;
use std::fmt;
use std::ops::Range;

pub(super) struct Spanned<T> {
    pub(super) value: T,
    pub(super) span: Range<usize>,
}

impl<T: PartialEq> PartialEq for Spanned<T> {
    fn eq(&self, rhs: &Self) -> bool {
        self.value.eq(&rhs.value)
    }
}

impl<T: Eq> Eq for Spanned<T> {}

impl<T: PartialOrd> PartialOrd for Spanned<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl<T: Ord> Ord for Spanned<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl<T> fmt::Debug for Spanned<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{:?}@{:?}", self.value, self.span)
    }
}

pub(crate) fn get_or_insert<'a, K: Clone + Ord, V, F: FnOnce() -> V>(
    from: &'a mut Vec<(K, V)>,
    key: &K,
    default: F,
) -> &'a mut V {
    match from.binary_search_by_key(&key, |(k, _)| k) {
        Ok(i) => &mut from[i].1,
        Err(i) => {
            from.insert(i, (key.clone(), default()));
            &mut from[i].1
        }
    }
}
