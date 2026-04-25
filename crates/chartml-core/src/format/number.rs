/// d3-format compatible number formatter.
///
/// Spec mini-language: `[[fill]align][sign][symbol][0][width][,][.precision][~][type]`

#[derive(Debug, Clone)]
pub struct NumberFormatter {
    spec: FormatSpec,
}

#[derive(Debug, Clone)]
struct FormatSpec {
    fill: char,
    align: Align,
    sign: Sign,
    symbol: Symbol,
    _zero: bool,
    width: Option<usize>,
    comma: bool,
    precision: Option<usize>,
    trim: bool,
    format_type: FormatType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Align {
    Left,
    Right,
    Center,
    SignFirst,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Sign {
    Minus,
    Plus,
    Space,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Symbol {
    None,
    Dollar,
    Hash,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum FormatType {
    None,
    Decimal,
    Exponent,
    Fixed,
    General,
    Rounded,
    SiPrefix,
    Percentage,
    Hex,
    HexUpper,
    Octal,
    Binary,
}

fn is_align_char(c: char) -> bool {
    matches!(c, '<' | '>' | '^' | '=')
}

fn parse_spec(spec_str: &str) -> FormatSpec {
    let chars: Vec<char> = spec_str.chars().collect();
    let len = chars.len();
    let mut i = 0;

    // Defaults
    let mut fill = ' ';
    let mut align = Align::Right;
    let mut sign = Sign::Minus;
    let mut symbol = Symbol::None;
    let mut zero = false;
    let mut width: Option<usize> = None;
    let mut comma = false;
    let mut precision: Option<usize> = None;
    let mut trim = false;
    let mut format_type = FormatType::None;
    let mut explicit_align = false;

    // Parse fill+align: if second char is an align char, first char is fill
    if len >= 2 && is_align_char(chars[1]) {
        fill = chars[0];
        align = match chars[1] {
            '<' => Align::Left,
            '>' => Align::Right,
            '^' => Align::Center,
            '=' => Align::SignFirst,
            _ => unreachable!(),
        };
        explicit_align = true;
        i = 2;
    } else if len >= 1 && is_align_char(chars[0]) {
        align = match chars[0] {
            '<' => Align::Left,
            '>' => Align::Right,
            '^' => Align::Center,
            '=' => Align::SignFirst,
            _ => unreachable!(),
        };
        explicit_align = true;
        i = 1;
    }

    // Parse sign
    if i < len {
        match chars[i] {
            '-' => { sign = Sign::Minus; i += 1; }
            '+' => { sign = Sign::Plus; i += 1; }
            ' ' => { sign = Sign::Space; i += 1; }
            _ => {}
        }
    }

    // Parse symbol
    if i < len {
        match chars[i] {
            '$' => { symbol = Symbol::Dollar; i += 1; }
            '#' => { symbol = Symbol::Hash; i += 1; }
            _ => {}
        }
    }

    // Parse zero
    if i < len && chars[i] == '0' {
        // Only treat as zero-fill flag if followed by a digit (width) or end/non-digit
        // But we need to distinguish "0" as zero flag from "0" as part of precision etc.
        // In d3-format, 0 here means zero-fill and implies align = SignFirst if no explicit align
        zero = true;
        if !explicit_align {
            fill = '0';
            align = Align::SignFirst;
        }
        i += 1;
    }

    // Parse width (digits)
    {
        let start = i;
        while i < len && chars[i].is_ascii_digit() {
            i += 1;
        }
        if i > start {
            width = Some(chars[start..i].iter().collect::<String>().parse().expect("width slice contains only ASCII digits"));
        }
    }

    // Parse comma
    if i < len && chars[i] == ',' {
        comma = true;
        i += 1;
    }

    // Parse .precision
    if i < len && chars[i] == '.' {
        i += 1;
        let start = i;
        while i < len && chars[i].is_ascii_digit() {
            i += 1;
        }
        if i > start {
            precision = Some(chars[start..i].iter().collect::<String>().parse().expect("precision slice contains only ASCII digits"));
        } else {
            precision = Some(0);
        }
    }

    // Parse trim ~
    if i < len && chars[i] == '~' {
        trim = true;
        i += 1;
    }

    // Parse type
    if i < len {
        format_type = match chars[i] {
            'd' => FormatType::Decimal,
            'e' => FormatType::Exponent,
            'f' => FormatType::Fixed,
            'g' => FormatType::General,
            'r' => FormatType::Rounded,
            's' => FormatType::SiPrefix,
            '%' => FormatType::Percentage,
            'x' => FormatType::Hex,
            'X' => FormatType::HexUpper,
            'o' => FormatType::Octal,
            'b' => FormatType::Binary,
            _ => FormatType::None,
        };
    }

    FormatSpec {
        fill,
        align,
        sign,
        symbol,
        _zero: zero,
        width,
        comma,
        precision,
        trim,
        format_type,
    }
}

/// SI prefix table: (threshold, divisor, suffix)
const SI_PREFIXES: &[(f64, f64, &str)] = &[
    (1e24, 1e24, "Y"),
    (1e21, 1e21, "Z"),
    (1e18, 1e18, "E"),
    (1e15, 1e15, "P"),
    (1e12, 1e12, "T"),
    (1e9, 1e9, "G"),
    (1e6, 1e6, "M"),
    (1e3, 1e3, "k"),
    (1.0, 1.0, ""),
    (1e-3, 1e-3, "m"),
    (1e-6, 1e-6, "\u{03bc}"),
    (1e-9, 1e-9, "n"),
];

fn si_prefix(value: f64) -> (f64, &'static str) {
    let abs = value.abs();
    if abs == 0.0 {
        return (0.0, "");
    }
    for &(threshold, divisor, suffix) in SI_PREFIXES {
        if abs >= threshold {
            return (value / divisor, suffix);
        }
    }
    // Very small values: use nano
    (value / 1e-9, "n")
}

fn insert_commas(int_part: &str) -> String {
    let len = int_part.len();
    if len <= 3 {
        return int_part.to_string();
    }
    let mut result = String::with_capacity(len + len / 3);
    for (idx, ch) in int_part.chars().enumerate() {
        if idx > 0 && (len - idx).is_multiple_of(3) {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

fn trim_trailing_zeros(s: &str) -> String {
    if s.contains('.') {
        let trimmed = s.trim_end_matches('0');
        if trimmed.ends_with('.') {
            trimmed.strip_suffix('.').unwrap_or(trimmed).to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        s.to_string()
    }
}

fn format_with_commas(formatted: &str) -> String {
    // Split into integer part and decimal part (and possible suffix)
    // The formatted string might be like "1234.56" or "1234"
    if let Some(dot_pos) = formatted.find('.') {
        let int_part = &formatted[..dot_pos];
        let rest = &formatted[dot_pos..];
        format!("{}{}", insert_commas(int_part), rest)
    } else {
        insert_commas(formatted)
    }
}

impl NumberFormatter {
    pub fn new(spec_str: &str) -> Self {
        Self {
            spec: parse_spec(spec_str),
        }
    }

    pub fn format(&self, value: f64) -> String {
        let spec = &self.spec;
        let is_negative = value < 0.0;
        let abs_value = value.abs();

        // Format the core number (without sign, symbol, padding)
        let (mut formatted, suffix) = self.format_core(abs_value);

        // Apply trim
        if spec.trim {
            formatted = trim_trailing_zeros(&formatted);
        }

        // Apply comma grouping
        if spec.comma {
            formatted = format_with_commas(&formatted);
        }

        // Append suffix (for SI prefix, percentage)
        formatted.push_str(&suffix);

        // Build sign string
        let sign_str = if is_negative {
            "-"
        } else {
            match spec.sign {
                Sign::Plus => "+",
                Sign::Space => " ",
                Sign::Minus => "",
            }
        };

        // Build symbol string
        let symbol_str = match spec.symbol {
            Symbol::Dollar => "$",
            // Hash prefix is handled inside format_core for hex/octal/binary
            Symbol::Hash => "",
            Symbol::None => "",
        };

        // In d3-format, for negative numbers with $, the sign comes before the symbol: -$1,234
        // For positive with sign: +$1,234
        // The order is: sign + symbol + number
        let body = format!("{}{}{}", sign_str, symbol_str, formatted);

        // Apply width/alignment
        self.apply_padding(&body, sign_str, symbol_str, &formatted)
    }

    fn format_core(&self, abs_value: f64) -> (String, String) {
        let spec = &self.spec;
        let precision = spec.precision;

        match spec.format_type {
            FormatType::Fixed => {
                let p = precision.unwrap_or(6);
                (format!("{:.prec$}", abs_value, prec = p), String::new())
            }
            FormatType::Decimal => {
                let rounded = abs_value.round() as i64;
                (format!("{}", rounded), String::new())
            }
            FormatType::Exponent => {
                let p = precision.unwrap_or(6);
                (format!("{:.prec$e}", abs_value, prec = p), String::new())
            }
            FormatType::General => {
                let p = precision.unwrap_or(6);
                // Use general formatting: scientific if exponent < -4 or >= precision
                let formatted = format_general(abs_value, p);
                (formatted, String::new())
            }
            FormatType::Percentage => {
                let pct_value = abs_value * 100.0;
                let p = precision.unwrap_or(6);
                (format!("{:.prec$}", pct_value, prec = p), "%".to_string())
            }
            FormatType::SiPrefix => {
                let (scaled, suffix) = si_prefix(abs_value);
                let p = precision.unwrap_or(6);
                // SI prefix with precision means significant digits
                if spec.precision.is_some() {
                    let formatted = format_significant(scaled, p);
                    (formatted, suffix.to_string())
                } else {
                    // Default: use enough digits
                    let formatted = format_default_si(scaled);
                    (formatted, suffix.to_string())
                }
            }
            FormatType::Hex => {
                let int_val = abs_value.round() as u64;
                let prefix = if spec.symbol == Symbol::Hash { "0x" } else { "" };
                (format!("{}{:x}", prefix, int_val), String::new())
            }
            FormatType::HexUpper => {
                let int_val = abs_value.round() as u64;
                let prefix = if spec.symbol == Symbol::Hash { "0X" } else { "" };
                (format!("{}{:X}", prefix, int_val), String::new())
            }
            FormatType::Octal => {
                let int_val = abs_value.round() as u64;
                let prefix = if spec.symbol == Symbol::Hash { "0o" } else { "" };
                (format!("{}{:o}", prefix, int_val), String::new())
            }
            FormatType::Binary => {
                let int_val = abs_value.round() as u64;
                let prefix = if spec.symbol == Symbol::Hash { "0b" } else { "" };
                (format!("{}{:b}", prefix, int_val), String::new())
            }
            FormatType::Rounded => {
                let p = precision.unwrap_or(6);
                (format_significant(abs_value, p), String::new())
            }
            FormatType::None => {
                // Like g, but trims trailing zeros
                let p = precision.unwrap_or(12);
                let formatted = format_general(abs_value, p);
                (trim_trailing_zeros(&formatted), String::new())
            }
        }
    }

    fn apply_padding(
        &self,
        body: &str,
        sign_str: &str,
        symbol_str: &str,
        number_part: &str,
    ) -> String {
        let spec = &self.spec;
        let w = match spec.width {
            Some(w) => w,
            None => return body.to_string(),
        };

        let body_len = body.chars().count();
        if body_len >= w {
            return body.to_string();
        }

        let pad_len = w - body_len;
        let padding: String = std::iter::repeat_n(spec.fill, pad_len).collect();

        match spec.align {
            Align::Left => format!("{}{}", body, padding),
            Align::Right => format!("{}{}", padding, body),
            Align::Center => {
                let left = pad_len / 2;
                let right = pad_len - left;
                let lpad: String = std::iter::repeat_n(spec.fill, left).collect();
                let rpad: String = std::iter::repeat_n(spec.fill, right).collect();
                format!("{}{}{}", lpad, body, rpad)
            }
            Align::SignFirst => {
                // Padding goes after sign+symbol but before the number
                format!("{}{}{}{}", sign_str, symbol_str, padding, number_part)
            }
        }
    }
}

fn format_general(value: f64, precision: usize) -> String {
    if value == 0.0 {
        return format!("{:.prec$}", 0.0, prec = precision.saturating_sub(1));
    }
    let exp = value.log10().floor() as i32;
    if exp < -4 || exp >= precision as i32 {
        format!("{:.prec$e}", value, prec = precision.saturating_sub(1))
    } else {
        let decimal_places = if precision as i32 > exp + 1 {
            (precision as i32 - exp - 1) as usize
        } else {
            0
        };
        format!("{:.prec$}", value, prec = decimal_places)
    }
}

fn format_significant(value: f64, sig_figs: usize) -> String {
    if value == 0.0 {
        return format!("{:.prec$}", 0.0, prec = sig_figs.saturating_sub(1));
    }
    let magnitude = value.abs().log10().floor() as i32;
    let decimal_places = if sig_figs as i32 > magnitude + 1 {
        (sig_figs as i32 - magnitude - 1) as usize
    } else {
        0
    };
    format!("{:.prec$}", value, prec = decimal_places)
}

fn format_default_si(value: f64) -> String {
    // For ~s without explicit precision, show the number with enough precision
    // to represent the value faithfully. Use up to 12 significant figures then trim.
    if value == 0.0 {
        return "0".to_string();
    }
    let magnitude = value.abs().log10().floor() as i32;
    let sig_figs = 12;
    let decimal_places = if sig_figs > magnitude + 1 {
        (sig_figs - magnitude - 1) as usize
    } else {
        0
    };
    format!("{:.prec$}", value, prec = decimal_places)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_currency_no_decimals() {
        assert_eq!(NumberFormatter::new("$,.0f").format(1234567.0), "$1,234,567");
    }

    #[test]
    fn test_currency_two_decimals() {
        assert_eq!(NumberFormatter::new("$,.2f").format(1234.5), "$1,234.50");
    }

    #[test]
    fn test_comma_no_decimals() {
        assert_eq!(NumberFormatter::new(",.0f").format(1234567.0), "1,234,567");
    }

    #[test]
    fn test_comma_two_decimals() {
        assert_eq!(NumberFormatter::new(",.2f").format(1234.56), "1,234.56");
    }

    #[test]
    fn test_percentage_zero_decimals() {
        assert_eq!(NumberFormatter::new(".0%").format(0.1234), "12%");
    }

    #[test]
    fn test_percentage_one_decimal() {
        assert_eq!(NumberFormatter::new(".1%").format(0.1234), "12.3%");
    }

    #[test]
    fn test_percentage_two_decimals() {
        assert_eq!(NumberFormatter::new(".2%").format(0.1234), "12.34%");
    }

    #[test]
    fn test_si_prefix_trim() {
        assert_eq!(NumberFormatter::new("~s").format(1234.0), "1.234k");
    }

    #[test]
    fn test_si_prefix_precision() {
        assert_eq!(NumberFormatter::new(".3s").format(1234.0), "1.23k");
    }

    #[test]
    fn test_si_prefix_mega_trim() {
        assert_eq!(NumberFormatter::new("~s").format(1234567.0), "1.234567M");
    }

    #[test]
    fn test_decimal() {
        assert_eq!(NumberFormatter::new("d").format(1234.0), "1234");
    }

    #[test]
    fn test_sign_comma() {
        assert_eq!(NumberFormatter::new("+,.0f").format(1234.0), "+1,234");
    }

    #[test]
    fn test_zero_fixed() {
        assert_eq!(NumberFormatter::new(".0f").format(0.0), "0");
    }

    #[test]
    fn test_zero_currency() {
        assert_eq!(NumberFormatter::new("$,.0f").format(0.0), "$0");
    }

    #[test]
    fn test_precision_fixed() {
        assert_eq!(NumberFormatter::new(".2f").format(4.56789), "4.57");
    }

    #[test]
    fn test_negative_currency() {
        assert_eq!(NumberFormatter::new("$,.0f").format(-1234.0), "-$1,234");
    }

    #[test]
    fn test_si_prefix_milli() {
        assert_eq!(NumberFormatter::new("~s").format(0.001234), "1.234m");
    }

    #[test]
    fn test_si_prefix_giga() {
        assert_eq!(NumberFormatter::new("~s").format(1e9), "1G");
    }

    #[test]
    fn test_insert_commas() {
        assert_eq!(insert_commas("1"), "1");
        assert_eq!(insert_commas("12"), "12");
        assert_eq!(insert_commas("123"), "123");
        assert_eq!(insert_commas("1234"), "1,234");
        assert_eq!(insert_commas("12345"), "12,345");
        assert_eq!(insert_commas("123456"), "123,456");
        assert_eq!(insert_commas("1234567"), "1,234,567");
    }

    #[test]
    fn test_parse_spec_basic() {
        let spec = parse_spec("$,.2f");
        assert_eq!(spec.symbol, Symbol::Dollar);
        assert!(spec.comma);
        assert_eq!(spec.precision, Some(2));
        assert_eq!(spec.format_type, FormatType::Fixed);
    }

    #[test]
    fn test_parse_spec_percentage() {
        let spec = parse_spec(".1%");
        assert_eq!(spec.precision, Some(1));
        assert_eq!(spec.format_type, FormatType::Percentage);
    }

    #[test]
    fn test_parse_spec_si() {
        let spec = parse_spec("~s");
        assert!(spec.trim);
        assert_eq!(spec.format_type, FormatType::SiPrefix);
    }

    #[test]
    fn test_width_padding() {
        assert_eq!(NumberFormatter::new("10d").format(42.0), "        42");
    }

    #[test]
    fn test_zero_padding() {
        assert_eq!(NumberFormatter::new("010d").format(42.0), "0000000042");
    }

    #[test]
    fn test_left_align() {
        assert_eq!(NumberFormatter::new("<10d").format(42.0), "42        ");
    }
}
