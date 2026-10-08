//! AppMeasurement products syntax from Adobe's products and merchandising guides.
//!
//! Sources:
//! https://experienceleague.adobe.com/en/docs/analytics/implementation/vars/page-vars/products
//! https://experienceleague.adobe.com/en/docs/analytics/implementation/vars/page-vars/evar-merchandising
//! https://experienceleague.adobe.com/en/docs/analytics/implementation/vars/page-vars/events/events-overview

/// Validate a decoded AppMeasurement products value. Account configuration,
/// purchase context, and delimiters that form another valid product are outside
/// a single string's syntax. Adobe says "64k bytes" without defining the unit;
/// this advisory uses 64,000 bytes and records the unit ambiguity in the audit.
pub(crate) fn validate_products(value: &str) -> Option<String> {
    if value.len() > 64_000 {
        return Some("The products string exceeds the advisory 64,000-byte interpretation of Adobe's 64k-byte maximum.".into());
    }
    for (index, product) in value.split(',').enumerate() {
        let row = index + 1;
        let cells: Vec<_> = product.split(';').collect();
        if !(2..=6).contains(&cells.len()) {
            return Some(format!(
                "Product {row} needs two to six semicolon-separated fields."
            ));
        }
        if cells[1].is_empty() {
            return Some(format!("Product {row} has no required product name."));
        }
        for (cell, name) in [(cells[0], "category"), (cells[1], "name")] {
            if cell.len() > 100 {
                return Some(format!("Product {row} {name} exceeds 100 UTF-8 bytes."));
            }
            if cell.contains('|') {
                return Some(format!("Product {row} {name} contains a pipe delimiter."));
            }
        }
        for (position, name) in [(2, "quantity"), (3, "price")] {
            if let Some(cell) = cells.get(position)
                && !cell.is_empty()
                && !numeric(cell)
            {
                return Some(format!(
                    "Product {row} {name} needs a numeric value without a currency symbol."
                ));
            }
        }
        if let Some(events) = cells.get(4)
            && !events.is_empty()
        {
            for event in events.split('|') {
                if !product_event(event) {
                    return Some(format!(
                        "Product {row} has an invalid event name, value, or serialization suffix."
                    ));
                }
            }
        }
        if let Some(evars) = cells.get(5)
            && !evars.is_empty()
        {
            for evar in evars.split('|') {
                let Some((name, value)) = evar.split_once('=') else {
                    return Some(format!(
                        "Product {row} merchandising eVars need name=value."
                    ));
                };
                if !numbered_name(name, "eVar", 250) {
                    return Some(format!(
                        "Product {row} merchandising eVar names must be eVar1 through eVar250."
                    ));
                }
                if value.len() > 255 {
                    return Some(format!(
                        "Product {row} merchandising eVar value exceeds 255 UTF-8 bytes."
                    ));
                }
            }
        }
    }
    None
}

fn product_event(value: &str) -> bool {
    // Adobe's XDM-to-AppMeasurement example emits event10=2:abcd. Serialization
    // IDs remain opaque because the current guide supplies no complete grammar.
    let event = match value.split_once(':') {
        Some((event, id)) if !id.is_empty() => event,
        Some(_) => return false,
        None => value,
    };
    let name = match event.split_once('=') {
        Some((name, amount)) if numeric(amount) => name,
        Some(_) => return false,
        None => event,
    };
    numbered_name(name, "event", 1000)
}

fn numbered_name(value: &str, prefix: &str, maximum: u16) -> bool {
    let Some(number) = value.strip_prefix(prefix) else {
        return false;
    };
    !number.starts_with('0')
        && number.bytes().all(|byte| byte.is_ascii_digit())
        && number
            .parse::<u16>()
            .is_ok_and(|number| (1..=maximum).contains(&number))
}

fn numeric(value: &str) -> bool {
    // Keep sign, fractional quantities, and exponent notation valid. The
    // AppMeasurement guide does not specify XDM's integer restriction for this
    // string transport or impose a positive-only rule on refunds.
    let unsigned = value.strip_prefix(['+', '-']).unwrap_or(value);
    let (mantissa, exponent) = match unsigned.find(['e', 'E']) {
        Some(index) => (&unsigned[..index], Some(&unsigned[index + 1..])),
        None => (unsigned, None),
    };
    let mut digits = 0;
    let mut dots = 0;
    for byte in mantissa.bytes() {
        if byte.is_ascii_digit() {
            digits += 1;
        } else if byte == b'.' {
            dots += 1;
        } else {
            return false;
        }
    }
    if digits == 0 || dots > 1 {
        return false;
    }
    exponent.is_none_or(|exponent| {
        let exponent = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        !exponent.is_empty() && exponent.bytes().all(|byte| byte.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::validate_products;

    #[test]
    fn adobe_documented_product_examples_are_valid() {
        for products in [
            "Example category;Example product",
            ";Example product",
            "Example category;Example product 1,;Example product 2",
            ";Example product;1;6.99",
            ";Example product 1;9;26.91,Example category;Example product 2;4;9.96",
            ";Example product;1;4.20;event1=2.3|event2=5",
            ";Example product;1;6.69;;eVar1=Merchandising value",
            ";Example product;;;;eVar1=Merchandising value",
            ";Bahama Shirt;3;12.99;event4|event10=2:abcd;eVar10=green|eVar33=large",
        ] {
            assert_eq!(validate_products(products), None, "{products}");
        }
    }

    #[test]
    fn malformed_delimiters_and_missing_names_are_rejected() {
        for products in [
            "",
            "Product",
            ";",
            "Category;;1;2",
            ";Product,",
            ",;Product",
            ";Product;;3;;eVar1=value;extra",
            ";Product|Other",
            "Cat|Other;Product",
            ";Product;eVar1=value",
            ";Product;;;;eVar1=value|unassigned",
        ] {
            assert!(validate_products(products).is_some(), "{products}");
        }
    }

    #[test]
    fn quantity_and_price_reject_non_numeric_values() {
        for products in [
            ";Product;one;2.99",
            ";Product;1;$2.99",
            ";Product;1;NaN",
            ";Product;1;Infinity",
            ";Product;1;2.3.4",
            ";Product;1;1e",
        ] {
            assert!(validate_products(products).is_some(), "{products}");
        }
        for products in [";Product;-1;-9.99", ";Product;.5;0", ";Product;0;1e2"] {
            assert_eq!(validate_products(products), None, "{products}");
        }
    }

    #[test]
    fn event_and_evar_number_boundaries_and_serialization_are_checked() {
        for products in [
            ";Product;;;;eVar1=x|eVar250=y=z",
            ";Product;;;event1|event1000=-2.5:order-id",
            ";Product;;;event1:ID;eVar1=",
        ] {
            assert_eq!(validate_products(products), None, "{products}");
        }
        for products in [
            ";Product;;;event0",
            ";Product;;;event1001",
            ";Product;;;Event1",
            ";Product;;;event1=",
            ";Product;;;event1:",
            ";Product;;;event1=two",
            ";Product;;;event1|",
            ";Product;;;|event1",
            ";Product;;;;eVar0=x",
            ";Product;;;;eVar251=x",
            ";Product;;;;evar1=x",
            ";Product;;;;eVar1",
            ";Product;;;;eVar1=x|",
        ] {
            assert!(validate_products(products).is_some(), "{products}");
        }
    }

    #[test]
    fn name_and_category_limits_count_utf8_bytes() {
        for prefix in ["a".repeat(100), "é".repeat(50)] {
            assert_eq!(validate_products(&format!("{prefix};{prefix}")), None);
        }
        for over_limit in ["a".repeat(101), "é".repeat(51)] {
            assert!(validate_products(&format!(";{over_limit}")).is_some());
            assert!(validate_products(&format!("{over_limit};Product")).is_some());
        }
    }

    #[test]
    fn whole_string_limit_counts_decoded_utf8_bytes() {
        let products = format!("{};ppp", ";p,".repeat(21_332));
        assert_eq!(products.len(), 64_000);
        assert_eq!(validate_products(&products), None);
        assert!(validate_products(&(products + "x")).is_some());
    }

    #[test]
    fn merchandising_evar_values_respect_the_shared_evar_byte_limit() {
        for value in ["a".repeat(255), format!("{}a", "é".repeat(127))] {
            assert_eq!(
                validate_products(&format!(";Product;;;;eVar250={value}")),
                None
            );
        }
        for value in ["a".repeat(256), "é".repeat(128)] {
            assert!(validate_products(&format!(";Product;;;;eVar250={value}")).is_some());
        }
    }
}
