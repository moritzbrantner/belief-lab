//! Calendar dates at the precision a source actually supports (`YYYY`, `YYYY-MM` or `YYYY-MM-DD`).
//! A coarse date covers an interval, so two dates are ordered only when their intervals are disjoint.

use std::error::Error;

use super::invalid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PartialDate {
    first: (u16, u8, u8),
    last: (u16, u8, u8),
}

impl PartialDate {
    pub(super) fn parse(field: &str, value: &str) -> Result<Self, Box<dyn Error>> {
        let bad = || invalid(format!("{field} must be YYYY, YYYY-MM or YYYY-MM-DD"));
        let parts = value.split('-').collect::<Vec<_>>();
        let number = |text: &str, digits: usize| {
            (text.len() == digits && text.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| text.parse::<u16>().ok())
                .flatten()
        };
        let year = parts
            .first()
            .and_then(|text| number(text, 4))
            .ok_or_else(bad)?;
        let month = match parts.get(1) {
            Some(text) => Some(
                number(text, 2)
                    .filter(|month| (1..=12).contains(month))
                    .ok_or_else(bad)? as u8,
            ),
            None => None,
        };
        let day = match parts.get(2) {
            Some(text) => {
                let day = number(text, 2).ok_or_else(bad)? as u8;
                if day == 0 || day > days_in_month(year, month.unwrap_or(1)) {
                    return Err(bad());
                }
                Some(day)
            }
            None => None,
        };
        if parts.len() > 3 {
            return Err(bad());
        }
        Ok(match (month, day) {
            (None, _) => Self {
                first: (year, 1, 1),
                last: (year, 12, 31),
            },
            (Some(month), None) => Self {
                first: (year, month, 1),
                last: (year, month, days_in_month(year, month)),
            },
            (Some(month), Some(day)) => Self {
                first: (year, month, day),
                last: (year, month, day),
            },
        })
    }

    /// True only when every day this date may denote precedes every day `other` may denote.
    pub(super) fn certainly_before(self, other: Self) -> bool {
        self.last < other.first
    }

    /// True when the two dates may denote the same day.
    pub(super) fn may_overlap(self, other: Self) -> bool {
        !self.certainly_before(other) && !other.certainly_before(self)
    }
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(value: &str) -> PartialDate {
        PartialDate::parse("date", value).unwrap()
    }

    #[test]
    fn coarse_dates_cover_intervals() {
        assert!(date("2019").certainly_before(date("2020-01-01")));
        assert!(date("2020").may_overlap(date("2020-06")));
        assert!(date("2020-02").may_overlap(date("2020-02-29")));
        assert!(!date("2021-03-01").certainly_before(date("2021-03-01")));
    }

    #[test]
    fn rejects_invented_or_malformed_dates() {
        for value in [
            "",
            "20",
            "2021-13",
            "2021-02-29",
            "2021-1-01",
            "2021-01-01T00:00",
        ] {
            assert!(PartialDate::parse("date", value).is_err(), "{value}");
        }
    }
}
