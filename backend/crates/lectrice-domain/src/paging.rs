use serde::Serialize;

/// Each read method picks its own defaults for missing values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ListOptions {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl ListOptions {
    /// The page size when the caller gives none.
    pub const DEFAULT_LIMIT: usize = 20;
    /// The largest page a caller may ask for.
    pub const MAX_LIMIT: usize = 100;

    pub fn limit_or_default(&self) -> usize {
        self.limit.unwrap_or(Self::DEFAULT_LIMIT)
    }

    /// A limit given must be between 1 and [`Self::MAX_LIMIT`].
    pub fn has_valid_limit(&self) -> bool {
        self.limit.is_none_or(|limit| (1..=Self::MAX_LIMIT).contains(&limit))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub limit: usize,
    pub offset: usize,
}
