/// Pagination parameters for an ordered listing.
///
/// The limit is guaranteed to be between 1 and `MAX_LIMIT`, inclusive. Each
/// service specifies `MAX_LIMIT` through a type alias. Pages with different
/// maximum limits are distinct types and cannot be used interchangeably.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Page<const MAX_LIMIT: usize> {
    /// The number of items to skip before returning results.
    offset: usize,

    /// The maximum number of items to return.
    limit: usize,
}

impl<const MAX_LIMIT: usize> Page<MAX_LIMIT> {
    pub fn new(offset: usize, limit: usize) -> Result<Self, PageError> {
        if limit == 0 || limit > MAX_LIMIT {
            return Err(PageError::LimitOutOfRange {
                limit,
                max_limit: MAX_LIMIT,
            });
        }
        Ok(Self { offset, limit })
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PageError {
    #[error("a page must request 1 to {max_limit} items, got {limit}")]
    LimitOutOfRange { limit: usize, max_limit: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_limits_within_bounds() {
        assert!(Page::<50>::new(0, 1).is_ok());
        assert_eq!(Page::<50>::new(100, 50).unwrap().offset(), 100);
    }

    #[test]
    fn rejects_limits_out_of_bounds() {
        assert_eq!(
            Page::<50>::new(0, 0),
            Err(PageError::LimitOutOfRange {
                limit: 0,
                max_limit: 50
            })
        );
        assert_eq!(
            Page::<20>::new(0, 21),
            Err(PageError::LimitOutOfRange {
                limit: 21,
                max_limit: 20
            })
        );
    }
}
