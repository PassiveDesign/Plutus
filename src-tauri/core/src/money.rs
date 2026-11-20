//! Money: Australian dollars as an exact integer number of cents.
//!
//! Plutus is AUD-only (see docs/DECISIONS.md). Amounts are never floating
//! point. Parsing is strict: anything ambiguous or that would need rounding is
//! an error, never a guess.
//!
//! Values are limited to ±[`Money::MAX_CENTS`] (2^53 − 1 cents, about ±$90
//! trillion) so they survive the trip to JavaScript, whose numbers are only
//! exact up to 2^53.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// An amount in AUD, stored as cents. Serialises as a plain integer of cents.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Money(i64);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MoneyError {
    #[error("amount is empty")]
    Empty,
    #[error("not a valid amount: {0:?}")]
    Invalid(String),
    #[error("amount has more than 2 decimal places: {0:?}")]
    TooManyDecimals(String),
    #[error("amount is too large: {0:?}")]
    OutOfRange(String),
}

impl MoneyError {
    /// Short message suitable for showing under a form field.
    pub fn friendly(&self) -> &'static str {
        match self {
            MoneyError::Empty => "Enter an amount.",
            MoneyError::Invalid(_) => "Enter an amount like 1,234.56.",
            MoneyError::TooManyDecimals(_) => "Amounts can have at most 2 decimal places.",
            MoneyError::OutOfRange(_) => "That amount is too large.",
        }
    }
}

impl Money {
    pub const ZERO: Money = Money(0);
    /// Largest magnitude allowed, in cents (JavaScript's `Number.MAX_SAFE_INTEGER`).
    pub const MAX_CENTS: i64 = (1 << 53) - 1;

    /// From a raw cent value (e.g. read back from the database).
    pub const fn from_cents(cents: i64) -> Self {
        Money(cents)
    }

    pub const fn cents(self) -> i64 {
        self.0
    }

    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    pub const fn is_zero(self) -> bool {
        self.0 == 0
    }

    fn bounded(cents: i64) -> Option<Self> {
        (cents.abs() <= Self::MAX_CENTS).then_some(Money(cents))
    }

    /// `None` if the result would exceed ±[`Money::MAX_CENTS`].
    pub fn checked_add(self, other: Money) -> Option<Money> {
        self.0.checked_add(other.0).and_then(Self::bounded)
    }

    /// `None` if the result would exceed ±[`Money::MAX_CENTS`].
    pub fn checked_sub(self, other: Money) -> Option<Money> {
        self.0.checked_sub(other.0).and_then(Self::bounded)
    }

    pub fn checked_neg(self) -> Option<Money> {
        self.0.checked_neg().and_then(Self::bounded)
    }

    pub fn abs(self) -> Option<Money> {
        self.0.checked_abs().and_then(Self::bounded)
    }

    /// Sum of all amounts, or `None` on overflow.
    pub fn checked_sum<I: IntoIterator<Item = Money>>(items: I) -> Option<Money> {
        items
            .into_iter()
            .try_fold(Money::ZERO, |acc, m| acc.checked_add(m))
    }

    /// Parse user or file input. Accepted forms (AUD only):
    ///
    /// - `12`, `12.3`, `12.30`, `.50`, `0.5`
    /// - thousands separators, correctly grouped: `1,234,567.89`
    /// - dollar sign: `$12.30`
    /// - negatives: `-12.30`, `-$12.30`, `$-12.30`, `(12.30)`, `($12.30)`
    /// - explicit plus: `+12.30`
    /// - surrounding whitespace, and spaces after the sign or `$`
    /// - extra decimal places only if they are zeros: `12.300`
    ///
    /// Rejected: `1.234` (needs rounding), `12,34` (bad grouping), `1.2.3`,
    /// `--5`, `(-5)`, `12.`, letters, other currencies' symbols.
    pub fn parse(input: &str) -> Result<Money, MoneyError> {
        let invalid = || MoneyError::Invalid(input.to_string());
        let mut s = input.trim();
        if s.is_empty() {
            return Err(MoneyError::Empty);
        }

        let mut negative = false;
        let parens = s.starts_with('(') || s.ends_with(')');
        if parens {
            s = s
                .strip_prefix('(')
                .and_then(|r| r.strip_suffix(')'))
                .ok_or_else(invalid)?
                .trim();
            negative = true;
        }

        // Optional sign and optional `$`, in either order, at most one each.
        let (mut seen_sign, mut seen_dollar) = (false, false);
        loop {
            if !seen_sign && !parens {
                if let Some(rest) = s.strip_prefix('-') {
                    negative = true;
                    seen_sign = true;
                    s = rest.trim_start();
                    continue;
                }
                if let Some(rest) = s.strip_prefix('+') {
                    seen_sign = true;
                    s = rest.trim_start();
                    continue;
                }
            }
            if !seen_dollar && let Some(rest) = s.strip_prefix('$') {
                seen_dollar = true;
                s = rest.trim_start();
                continue;
            }
            break;
        }

        let (int_part, frac_part) = match s.split_once('.') {
            Some((i, f)) => (i, Some(f)),
            None => (s, None),
        };

        // Integer part: plain digits, or digits grouped by commas in threes.
        let int_digits: String = if int_part.contains(',') {
            let mut groups = int_part.split(',');
            let first = groups.next().unwrap_or_default();
            let first_ok = (1..=3).contains(&first.len()) && all_digits(first);
            let rest: Vec<&str> = groups.collect();
            if !first_ok || rest.iter().any(|g| g.len() != 3 || !all_digits(g)) {
                return Err(invalid());
            }
            std::iter::once(first).chain(rest).collect()
        } else {
            if !all_digits(int_part) {
                return Err(invalid());
            }
            int_part.to_string()
        };

        // Fraction: 1+ digits after a '.', at most 2 significant.
        let frac_cents: i64 = match frac_part {
            None => 0,
            Some(f) => {
                if f.is_empty() || !all_digits(f) {
                    return Err(invalid());
                }
                if f.len() > 2 && f[2..].bytes().any(|b| b != b'0') {
                    return Err(MoneyError::TooManyDecimals(input.to_string()));
                }
                let two: String = f.chars().chain(std::iter::repeat('0')).take(2).collect();
                two.parse().map_err(|_| invalid())?
            }
        };

        if int_digits.is_empty() && frac_part.is_none() {
            return Err(invalid());
        }

        let too_big = || MoneyError::OutOfRange(input.to_string());
        let trimmed = int_digits.trim_start_matches('0');
        if trimmed.len() > 18 {
            return Err(too_big());
        }
        let dollars: i64 = if trimmed.is_empty() {
            0
        } else {
            trimmed.parse().map_err(|_| too_big())?
        };
        let cents = dollars
            .checked_mul(100)
            .and_then(|c| c.checked_add(frac_cents))
            .filter(|c| *c <= Self::MAX_CENTS)
            .ok_or_else(too_big)?;

        Ok(Money(if negative { -cents } else { cents }))
    }

    /// Plain decimal with no symbol or separators, e.g. `-1234.50`.
    /// Use for CSV export and editable input fields.
    pub fn to_plain_string(self) -> String {
        let abs = self.0.unsigned_abs();
        let sign = if self.0 < 0 { "-" } else { "" };
        format!("{sign}{}.{:02}", abs / 100, abs % 100)
    }
}

fn all_digits(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit())
}

/// Display form: `$1,234.56`, `-$12.30`, `$0.00`.
impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let abs = self.0.unsigned_abs();
        let digits = (abs / 100).to_string();
        let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
        for (i, ch) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                grouped.push(',');
            }
            grouped.push(ch);
        }
        let sign = if self.0 < 0 { "-" } else { "" };
        write!(f, "{sign}${grouped}.{:02}", abs % 100)
    }
}

impl FromStr for Money {
    type Err = MoneyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Money::parse(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> i64 {
        Money::parse(s)
            .unwrap_or_else(|e| panic!("{s:?} should parse: {e}"))
            .cents()
    }

    fn err(s: &str) -> MoneyError {
        Money::parse(s).expect_err(&format!("{s:?} should be rejected"))
    }

    #[test]
    fn parses_plain_numbers() {
        assert_eq!(p("12"), 1200);
        assert_eq!(p("12.3"), 1230);
        assert_eq!(p("12.30"), 1230);
        assert_eq!(p("12.05"), 1205);
        assert_eq!(p("0.5"), 50);
        assert_eq!(p(".50"), 50);
        assert_eq!(p(".5"), 50);
        assert_eq!(p("0"), 0);
        assert_eq!(p("0.00"), 0);
        assert_eq!(p("007.10"), 710);
    }

    #[test]
    fn parses_dollar_sign_and_thousands() {
        assert_eq!(p("$12.30"), 1230);
        assert_eq!(p("1,234.56"), 123_456);
        assert_eq!(p("$1,234,567.89"), 123_456_789);
        assert_eq!(p("999,999"), 99_999_900);
        assert_eq!(p("  $ 1,000  "), 100_000);
    }

    #[test]
    fn parses_negatives() {
        assert_eq!(p("-12.30"), -1230);
        assert_eq!(p("-$12.30"), -1230);
        assert_eq!(p("$-12.30"), -1230);
        assert_eq!(p("- 12.30"), -1230);
        assert_eq!(p("(12.30)"), -1230);
        assert_eq!(p("($1,234.56)"), -123_456);
        assert_eq!(p("( 5 )"), -500);
        assert_eq!(p("+12.30"), 1230);
        assert_eq!(p("-0"), 0);
        assert_eq!(p("-0.00"), 0);
    }

    #[test]
    fn extra_zero_decimals_are_fine_but_rounding_is_not() {
        assert_eq!(p("12.300"), 1230);
        assert_eq!(p("12.3000"), 1230);
        assert!(matches!(err("1.234"), MoneyError::TooManyDecimals(_)));
        assert!(matches!(err("1.001"), MoneyError::TooManyDecimals(_)));
        assert!(matches!(err("-0.005"), MoneyError::TooManyDecimals(_)));
    }

    #[test]
    fn rejects_malformed_input() {
        assert_eq!(err(""), MoneyError::Empty);
        assert_eq!(err("   "), MoneyError::Empty);
        for bad in [
            "abc", "12a", "1.2.3", "12.", ".", "$", "-", "--5", "-$-5", "$$5", "(-5)", "(5", "5)",
            "()", "12,34", "1,2345", ",123", "1234,567", "1,234,", "1,,234", "1 234", "€12",
            "12 AUD", "1e3", "0x10", "١٢", "12.3a", "+-5",
        ] {
            assert!(
                matches!(Money::parse(bad), Err(MoneyError::Invalid(_))),
                "{bad:?} should be Invalid, got {:?}",
                Money::parse(bad)
            );
        }
    }

    #[test]
    fn range_limits() {
        let max = Money::MAX_CENTS; // 90,071,992,547,409.91
        assert_eq!(p("90071992547409.91"), max);
        assert_eq!(p("-90,071,992,547,409.91"), -max);
        assert!(matches!(
            err("90071992547409.92"),
            MoneyError::OutOfRange(_)
        ));
        assert!(matches!(
            err("99999999999999999999999"),
            MoneyError::OutOfRange(_)
        ));
        // Leading zeros don't count toward the size limit.
        assert_eq!(p("0000000000000000000000012.00"), 1200);
    }

    #[test]
    fn display_format() {
        let f = |c: i64| Money::from_cents(c).to_string();
        assert_eq!(f(0), "$0.00");
        assert_eq!(f(5), "$0.05");
        assert_eq!(f(-5), "-$0.05");
        assert_eq!(f(100), "$1.00");
        assert_eq!(f(99_999), "$999.99");
        assert_eq!(f(100_000), "$1,000.00");
        assert_eq!(f(123_456_789), "$1,234,567.89");
        assert_eq!(f(-123_456), "-$1,234.56");
        assert_eq!(f(i64::MIN), "-$92,233,720,368,547,758.08"); // no panic
    }

    #[test]
    fn plain_format() {
        let f = |c: i64| Money::from_cents(c).to_plain_string();
        assert_eq!(f(0), "0.00");
        assert_eq!(f(123_456), "1234.56");
        assert_eq!(f(-5), "-0.05");
    }

    #[test]
    fn display_and_plain_round_trip_through_parse() {
        for c in [
            0,
            1,
            -1,
            99,
            100,
            -12_345,
            123_456_789,
            Money::MAX_CENTS,
            -Money::MAX_CENTS,
        ] {
            let m = Money::from_cents(c);
            assert_eq!(Money::parse(&m.to_string()), Ok(m), "display {c}");
            assert_eq!(Money::parse(&m.to_plain_string()), Ok(m), "plain {c}");
        }
    }

    #[test]
    fn checked_arithmetic() {
        let a = Money::from_cents(1050);
        let b = Money::from_cents(250);
        assert_eq!(a.checked_add(b), Some(Money::from_cents(1300)));
        assert_eq!(b.checked_sub(a), Some(Money::from_cents(-800)));
        assert_eq!(a.checked_neg(), Some(Money::from_cents(-1050)));
        assert_eq!(Money::from_cents(-7).abs(), Some(Money::from_cents(7)));

        let max = Money::from_cents(Money::MAX_CENTS);
        assert_eq!(max.checked_add(Money::from_cents(1)), None);
        assert_eq!(
            max.checked_neg().unwrap().checked_sub(Money::from_cents(1)),
            None
        );
        assert_eq!(Money::from_cents(i64::MIN).checked_neg(), None);
        assert_eq!(Money::from_cents(i64::MIN).abs(), None);
    }

    #[test]
    fn checked_sum() {
        let items = [1050, -250, 1].map(Money::from_cents);
        assert_eq!(Money::checked_sum(items), Some(Money::from_cents(801)));
        assert_eq!(Money::checked_sum([]), Some(Money::ZERO));
        let big = [Money::from_cents(Money::MAX_CENTS), Money::from_cents(1)];
        assert_eq!(Money::checked_sum(big), None);
    }

    #[test]
    fn predicates_and_ordering() {
        assert!(Money::from_cents(-1).is_negative());
        assert!(!Money::ZERO.is_negative());
        assert!(Money::ZERO.is_zero());
        assert!(Money::from_cents(-1) < Money::ZERO);
        assert_eq!(Money::default(), Money::ZERO);
        assert_eq!("$4.20".parse::<Money>(), Ok(Money::from_cents(420)));
    }

    #[test]
    fn friendly_messages() {
        assert_eq!(err("").friendly(), "Enter an amount.");
        assert_eq!(err("abc").friendly(), "Enter an amount like 1,234.56.");
        assert_eq!(
            err("1.234").friendly(),
            "Amounts can have at most 2 decimal places."
        );
        assert_eq!(
            err("9".repeat(30).as_str()).friendly(),
            "That amount is too large."
        );
    }

    #[test]
    fn serialises_as_integer_cents() {
        let m = Money::from_cents(-123_456);
        assert_eq!(serde_json::to_string(&m).unwrap(), "-123456");
        assert_eq!(
            serde_json::from_str::<Money>("420").unwrap(),
            Money::from_cents(420)
        );
        assert!(
            serde_json::from_str::<Money>("4.20").is_err(),
            "floats must not deserialise"
        );
    }
}
