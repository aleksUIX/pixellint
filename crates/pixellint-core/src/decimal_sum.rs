//! Exact decimal sums for documented delimited amount relationships.
//!
//! Awin requires commission-group amounts to sum to the sale subtotal:
//! https://help.awin.com/developers/docs/commission-groups
//! Read 2026-10-07. Source arithmetic uses decimal values, not a tolerance.

use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Decimal {
    negative: bool,
    // Least-significant decimal digit first. No integer-size restriction.
    digits: Vec<u8>,
    scale: usize,
}

impl Decimal {
    fn parse(value: &str) -> Option<Self> {
        let (negative, value) = match value.as_bytes().first()? {
            b'-' => (true, &value[1..]),
            b'+' => (false, &value[1..]),
            _ => (false, value),
        };
        let (integer, fraction) = value.split_once('.').unwrap_or((value, ""));
        if integer.is_empty()
            || !integer.bytes().all(|digit| digit.is_ascii_digit())
            || !fraction.bytes().all(|digit| digit.is_ascii_digit())
            || (value.contains('.') && fraction.is_empty())
        {
            return None;
        }
        let mut decimal = Self {
            negative,
            digits: integer
                .bytes()
                .chain(fraction.bytes())
                .rev()
                .map(|digit| digit - b'0')
                .collect(),
            scale: fraction.len(),
        };
        decimal.normalize();
        Some(decimal)
    }

    fn normalize(&mut self) {
        while self.digits.last() == Some(&0) && self.digits.len() > 1 {
            self.digits.pop();
        }
        if self.digits == [0] {
            self.scale = 0;
            self.negative = false;
            return;
        }
        let remove = self
            .digits
            .iter()
            .take(self.scale)
            .take_while(|digit| **digit == 0)
            .count();
        if remove > 0 {
            self.digits.drain(..remove);
            self.scale -= remove;
        }
    }

    fn coefficient(&self) -> (&[u8], i128) {
        let zeros = self.digits.iter().take_while(|digit| **digit == 0).count();
        (&self.digits[zeros..], zeros as i128 - self.scale as i128)
    }

    fn add(&mut self, other: Self) {
        let scale = self.scale.max(other.scale);
        let shifted = |value: &Self| {
            let mut digits = std::iter::repeat_n(0, scale - value.scale)
                .chain(value.digits.iter().copied())
                .collect::<Vec<_>>();
            while digits.last() == Some(&0) && digits.len() > 1 {
                digits.pop();
            }
            digits
        };
        let mut left = shifted(self);
        let mut right = shifted(&other);
        if self.negative == other.negative {
            let mut carry = 0;
            let digits = (0..left.len().max(right.len()))
                .map(|index| {
                    let sum = left.get(index).copied().unwrap_or(0)
                        + right.get(index).copied().unwrap_or(0)
                        + carry;
                    carry = sum / 10;
                    sum % 10
                })
                .collect::<Vec<_>>();
            self.digits = digits;
            if carry > 0 {
                self.digits.push(carry);
            }
        } else {
            let ordering = left
                .len()
                .cmp(&right.len())
                .then_with(|| left.iter().rev().cmp(right.iter().rev()));
            if ordering == Ordering::Less {
                std::mem::swap(&mut left, &mut right);
                self.negative = other.negative;
            }
            let mut borrow = 0i8;
            self.digits = left
                .iter()
                .enumerate()
                .map(|(index, digit)| {
                    let difference =
                        *digit as i8 - right.get(index).copied().unwrap_or(0) as i8 - borrow;
                    borrow = i8::from(difference < 0);
                    if difference < 0 {
                        (difference + 10) as u8
                    } else {
                        difference as u8
                    }
                })
                .collect();
        }
        self.scale = scale;
        self.normalize();
    }
}

/// Returns no opinion for unresolved or syntactically invalid amount values.
/// Their scalar contracts retain responsibility for syntax diagnostics.
pub(crate) fn sum_matches(
    parts: &str,
    total: &str,
    separator: &str,
    value_separator: &str,
) -> Option<bool> {
    if separator.is_empty() || value_separator.is_empty() || parts.is_empty() {
        return None;
    }
    let mut sum = Decimal::parse("0")?;
    for part in parts.split(separator) {
        let (label, value) = part.split_once(value_separator)?;
        if label.is_empty() {
            return None;
        }
        sum.add(Decimal::parse(value)?);
    }
    // A total may use exponent text accepted by a floating-point scalar
    // format. Compare canonical coefficient and exponent without expanding
    // a gigantic power of ten or rounding through binary floating point.
    let (mantissa, exponent) = total.split_once(['e', 'E']).unwrap_or((total, "0"));
    let total = Decimal::parse(mantissa)?;
    let exponent_digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
    if exponent_digits.is_empty() || !exponent_digits.bytes().all(|digit| digit.is_ascii_digit()) {
        return None;
    }
    if sum.digits == [0] || total.digits == [0] {
        return Some(sum.digits == [0] && total.digits == [0]);
    }
    let (sum_coefficient, sum_power) = sum.coefficient();
    let (total_coefficient, total_power) = total.coefficient();
    if sum.negative != total.negative || sum_coefficient != total_coefficient {
        return Some(false);
    }
    // Inputs and decimal positions fit usize. An exponent outside i128
    // cannot equal the finite difference of their positions.
    Some(exponent.parse::<i128>().ok() == Some(sum_power - total_power))
}

#[cfg(test)]
mod tests {
    use super::sum_matches;

    #[test]
    fn source_example_and_decimal_fraction_boundaries_are_exact() {
        assert_eq!(
            sum_matches("CD:11.10|DVD:14.99", "26.09", "|", ":"),
            Some(true)
        );
        assert_eq!(
            sum_matches("CD:11.10|DVD:14.99", "26.10", "|", ":"),
            Some(false)
        );
        assert_eq!(sum_matches("A:0.1|B:0.2", "0.300000", "|", ":"), Some(true));
        assert_eq!(
            sum_matches("A:0.1|B:0.2", "0.30000000000000001", "|", ":"),
            Some(false)
        );
        assert_eq!(
            sum_matches("A:000.0100|B:0.990", "1.000", "|", ":"),
            Some(true)
        );
        assert_eq!(sum_matches("A:0|B:0.00", "-0", "|", ":"), Some(true));
        assert_eq!(sum_matches("A:1", "2e0", "|", ":"), Some(false));
        assert_eq!(sum_matches("A:1", "1e0", "|", ":"), Some(true));
        assert_eq!(sum_matches("A:1", "10e-1", "|", ":"), Some(true));
        assert_eq!(sum_matches("A:1000", "1E+3", "|", ":"), Some(true));
        assert_eq!(sum_matches("A:0.01", "1e-2", "|", ":"), Some(true));
    }

    #[test]
    fn amounts_have_no_machine_integer_or_decimal_precision_ceiling() {
        let nines = "9".repeat(1000);
        let target = format!("1{}", "0".repeat(1000));
        assert_eq!(
            sum_matches(&format!("A:{nines}|B:1"), &target, "|", ":"),
            Some(true)
        );
        let tiny = format!("0.{}1", "0".repeat(1000));
        let twice = format!("0.{}2", "0".repeat(1000));
        assert_eq!(
            sum_matches(&format!("A:{tiny}|B:{tiny}"), &twice, "|", ":"),
            Some(true)
        );
        assert_eq!(
            sum_matches("A:-1.01|B:2|C:-0.99", "0", "|", ":"),
            Some(true)
        );
        assert_eq!(
            sum_matches("A:-100.01|B:0.01", "-100", "|", ":"),
            Some(true)
        );
        assert_eq!(sum_matches("A:-0.01", "-0.0100", "|", ":"), Some(true));
        assert_eq!(
            sum_matches("A:1", &format!("1e{}", "9".repeat(1000)), "|", ":"),
            Some(false)
        );
        assert_eq!(
            sum_matches("A:1", &format!("1e-{}", "9".repeat(1000)), "|", ":"),
            Some(false)
        );
        assert_eq!(
            sum_matches("A:0", &format!("0e{}", "9".repeat(1000)), "|", ":"),
            Some(true)
        );
    }

    #[test]
    fn field_syntax_and_macros_remain_unknown_to_arithmetic() {
        for parts in [
            "",
            "A:[AMOUNT]",
            "A:1|",
            "A:1,000",
            "A:1e2",
            "A:",
            ":1",
            "A:1:2",
        ] {
            assert_eq!(sum_matches(parts, "1", "|", ":"), None, "{parts}");
        }
        assert_eq!(sum_matches("A:1", "{{TOTAL}}", "|", ":"), None);
        assert_eq!(sum_matches("A=1;B=2", "3", ";", "="), Some(true));
        assert_eq!(sum_matches("A:1", "1", "", ":"), None);
        assert_eq!(sum_matches("A:1", "1", "|", ""), None);
    }
}
