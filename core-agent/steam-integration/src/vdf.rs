//! Minimal parser for Valve's KeyValues ("VDF") text format, which Steam uses
//! for libraryfolders.vdf and appmanifest_*.acf. Deliberately tolerant and
//! flat-lookup-oriented: Aegis needs a handful of known string fields out of
//! these files, not a full document model, and Steam has been known to vary
//! whitespace and add fields between client versions. Escape handling covers
//! the two escapes that appear in practice (\" and \\).

/// One `"key" "value"` pair found anywhere in the document, with the path of
/// enclosing block keys that led to it, e.g. (["libraryfolders","0"], "path", "C:\\...").
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: Vec<String>,
    pub key: String,
    pub value: String,
}

pub fn parse(text: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    // A quoted string with no sibling value yet — pending block name or pending key.
    let mut pending: Option<String> = None;

    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let s = read_quoted(&mut chars);
                match pending.take() {
                    None => pending = Some(s),
                    Some(key) => {
                        entries.push(Entry {
                            path: stack.clone(),
                            key,
                            value: s,
                        });
                    }
                }
            }
            '{' => {
                // The pending string names the block we're entering.
                stack.push(pending.take().unwrap_or_default());
            }
            '}' => {
                stack.pop();
                pending = None;
            }
            '/' if chars.peek() == Some(&'/') => {
                // Line comment; skip to end of line.
                for c2 in chars.by_ref() {
                    if c2 == '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    entries
}

fn read_quoted(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut out = String::new();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(escaped) = chars.next() {
                    out.push(escaped);
                }
            }
            '"' => break,
            _ => out.push(c),
        }
    }
    out
}

/// Convenience: value of the first entry whose key matches, case-insensitively.
/// ACF field casing is not stable across Steam versions ("StateFlags" vs
/// "stateflags" have both been observed in the wild).
pub fn first_value<'a>(entries: &'a [Entry], key: &str) -> Option<&'a str> {
    entries
        .iter()
        .find(|e| e.key.eq_ignore_ascii_case(key))
        .map(|e| e.value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_ACF: &str = r#"
"AppState"
{
	"appid"		"1245620"
	"name"		"ELDEN RING"
	"StateFlags"		"4"
	"installdir"		"ELDEN RING"
	"UserConfig"
	{
		"language"		"english"
	}
}
"#;

    #[test]
    fn parses_top_level_fields() {
        let entries = parse(SAMPLE_ACF);
        assert_eq!(first_value(&entries, "appid"), Some("1245620"));
        assert_eq!(first_value(&entries, "name"), Some("ELDEN RING"));
        assert_eq!(first_value(&entries, "stateflags"), Some("4"));
    }

    #[test]
    fn tracks_nesting_path() {
        let entries = parse(SAMPLE_ACF);
        let lang = entries.iter().find(|e| e.key == "language").unwrap();
        assert_eq!(lang.path, vec!["AppState", "UserConfig"]);
        assert_eq!(lang.value, "english");
    }

    #[test]
    fn handles_escaped_backslashes_in_windows_paths() {
        let text = r#""libraryfolders" { "0" { "path" "C:\\Program Files (x86)\\Steam" } }"#;
        let entries = parse(text);
        assert_eq!(
            first_value(&entries, "path"),
            Some(r"C:\Program Files (x86)\Steam")
        );
    }

    #[test]
    fn survives_comments_and_stray_braces_in_strings() {
        let text = "// comment {\n\"k\" \"v { } w\"\n";
        let entries = parse(text);
        assert_eq!(first_value(&entries, "k"), Some("v { } w"));
    }
}
