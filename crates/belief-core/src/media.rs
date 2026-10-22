//! Where within a media source an evidence reference points. Times use the canonical rational
//! media-time form (`pts` ticks of `num/den` seconds); no derived seconds are stored.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::ModelError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaTimestamp {
    pts: i64,
    num: u32,
    den: u32,
}

impl MediaTimestamp {
    pub fn new(pts: i64, num: u32, den: u32) -> Result<Self, ModelError> {
        if num == 0 || den == 0 {
            return Err(ModelError::InvalidMedia("timebase must be positive".into()));
        }
        Ok(Self { pts, num, den })
    }

    pub fn pts(self) -> i64 {
        self.pts
    }

    pub fn timebase(self) -> (u32, u32) {
        (self.num, self.den)
    }

    /// Exact comparison across timebases: pts·num/den without rounding.
    pub fn exact_cmp(self, other: Self) -> Ordering {
        let left = i128::from(self.pts) * i128::from(self.num) * i128::from(other.den);
        let right = i128::from(other.pts) * i128::from(other.num) * i128::from(self.den);
        left.cmp(&right)
    }
}

/// A half-open `[start, end)` range of one media source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaSpan {
    start: MediaTimestamp,
    end: MediaTimestamp,
}

impl MediaSpan {
    pub fn new(start: MediaTimestamp, end: MediaTimestamp) -> Result<Self, ModelError> {
        if start.pts < 0 || start.exact_cmp(end) != Ordering::Less {
            return Err(ModelError::InvalidMedia(
                "media span needs a non-negative start before its end".into(),
            ));
        }
        Ok(Self { start, end })
    }

    pub fn start(self) -> MediaTimestamp {
        self.start
    }

    pub fn end(self) -> MediaTimestamp {
        self.end
    }
}

/// The span plus the producer scene records that cover it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaLocator {
    span: MediaSpan,
    scene_ids: Vec<String>,
}

impl MediaLocator {
    pub fn new(span: MediaSpan, scene_ids: Vec<String>) -> Result<Self, ModelError> {
        let unique = scene_ids.iter().collect::<BTreeSet<_>>();
        if unique.len() != scene_ids.len() || scene_ids.iter().any(|id| id.trim().is_empty()) {
            return Err(ModelError::InvalidMedia(
                "scene ids must be unique and non-empty".into(),
            ));
        }
        Ok(Self { span, scene_ids })
    }

    pub fn span(&self) -> MediaSpan {
        self.span
    }

    pub fn scene_ids(&self) -> &[String] {
        &self.scene_ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_compare_exactly_across_timebases_and_reject_empty_ranges() {
        let start = MediaTimestamp::new(90, 1, 90_000).unwrap();
        let end = MediaTimestamp::new(1, 1, 1000).unwrap();
        // 90/90000 s == 1/1000 s: an empty range.
        assert!(MediaSpan::new(start, end).is_err());
        let end = MediaTimestamp::new(2, 1, 1000).unwrap();
        assert!(MediaSpan::new(start, end).is_ok());
        assert!(MediaTimestamp::new(1, 0, 1).is_err());
        assert!(MediaSpan::new(MediaTimestamp::new(-1, 1, 1).unwrap(), end).is_err());
        let span = MediaSpan::new(start, end).unwrap();
        assert!(MediaLocator::new(span, vec!["s".into(), "s".into()]).is_err());
    }
}
