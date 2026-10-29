// Copyright (C) 2026 University of Bristol
// rs-graphics includes code translated from R, copyright (C) The R Core Team
// and others; see NOTICE.
//
// This file is part of rs-graphics, licensed under the GNU General Public
// License, version 3 or (at your option) any later version; see LICENSE.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Files generated from the code, checked to be up to date: the R-to-Rust
//! table (`src/guide/r_reference.md`, from the `call!`/`options!`/
//! `par_options!` declarations and R's arguments in
//! `tests/data/r_formals.txt`) and the charts shown in `charts`' docs
//! (`src/charts/*.svg`). A test fails when one is stale; run
//! `RS_GRAPHICS_BLESS=1 cargo test --test all generated_docs` to rewrite
//! them, then review the diff.

use std::collections::{BTreeMap, BTreeSet};

use rs_graphics::charts;
use rs_graphics::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Compares `content` with the committed file `rel`, or writes it when
/// `RS_GRAPHICS_BLESS` is set.
fn check_generated(rel: &str, content: &str) {
    let path = root().join(rel);
    if std::env::var_os("RS_GRAPHICS_BLESS").is_some() {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        return;
    }
    let committed = fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == content,
        "{rel} is out of date; run `RS_GRAPHICS_BLESS=1 cargo test --test all \
         generated_docs` and review the diff"
    );
}

/// Every `.rs` file under `src`, with its text.
fn sources() -> Vec<(PathBuf, String)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push((p.clone(), fs::read_to_string(&p).unwrap()));
            }
        }
    }
    let mut out = Vec::new();
    walk(&root().join("src"), &mut out);
    out
}

/// A setter: its Rust name and doc comment.
#[derive(Clone, Debug)]
struct Setter {
    name: String,
    doc: String,
    /// How it's written, when not as a setter `.name()`.
    rust: Option<String>,
}

/// The `///` lines just above line `i` (attributes in between skipped).
fn doc_above(lines: &[&str], i: usize) -> String {
    let mut doc = Vec::new();
    let mut j = i;
    while j > 0 {
        j -= 1;
        let t = lines[j].trim();
        if let Some(d) = t.strip_prefix("///") {
            doc.push(d.trim().to_string());
        } else if t.starts_with("#[") {
            continue;
        } else {
            break;
        }
    }
    doc.reverse();
    doc.join(" ")
}

/// Options structs declared with `options!` (and `Par`): struct name, the
/// builders it gives setters to, and its fields with setters.
fn options_structs(src: &[(PathBuf, String)]) -> Vec<(String, Vec<String>, Vec<Setter>)> {
    let mut out = Vec::new();
    for (_, text) in src {
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let t = lines[i].trim();
            let header = t
                .strip_prefix("pub struct ")
                .filter(|_| t.ends_with('{'))
                .map(|h| h.trim_end_matches('{').trim());
            let Some(header) = header else {
                i += 1;
                continue;
            };
            let (name, calls) = match header.split_once("=>") {
                Some((n, c)) => (
                    n.trim().to_string(),
                    c.trim()
                        .trim_matches(|c| c == '[' || c == ']')
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                ),
                None => (header.to_string(), Vec::new()),
            };
            // Only macro-declared structs: their fields carry `=> kind`.
            let indent = lines[i].len() - lines[i].trim_start().len();
            let mut fields = Vec::new();
            let mut is_options = false;
            i += 1;
            while i < lines.len() {
                let l = lines[i];
                let lt = l.trim();
                if lt == "}" && l.len() - l.trim_start().len() == indent {
                    break;
                }
                if let Some(rest) = lt.strip_prefix("pub ") {
                    if let Some((field, ty)) = rest.split_once(':') {
                        let field = field.trim();
                        if field.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                            // The kind is after `=>`, maybe on the next line.
                            let mut decl = ty.to_string();
                            if !decl.contains("=>") && i + 1 < lines.len() {
                                decl.push_str(lines[i + 1]);
                            }
                            if let Some((_, kind)) = decl.split_once("=>") {
                                is_options = true;
                                if !kind.trim().starts_with("no_setter") {
                                    fields.push(Setter {
                                        name: field.to_string(),
                                        doc: doc_above(&lines, i),
                                        rust: None,
                                    });
                                }
                            }
                        }
                    }
                }
                i += 1;
            }
            if is_options {
                out.push((name, calls, fields));
            }
            i += 1;
        }
    }
    out
}

/// The `call!` invocations: builder, R name, and argument setters.
fn calls(graphics: &str) -> Vec<(String, String, Vec<Setter>)> {
    let lines: Vec<&str> = graphics.lines().collect();
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim();
        let Some((builder, rest)) = t.split_once("(\"") else {
            continue;
        };
        if !builder.ends_with("Call") || !builder.chars().all(|c| c.is_ascii_alphanumeric()) {
            continue;
        }
        let Some((rname, _)) = rest.split_once("\")") else {
            continue;
        };
        let mut args = Vec::new();
        let mut j = i + 1;
        while j < lines.len() && !lines[j].trim().starts_with("args {") && lines[j].trim() != ");" {
            j += 1;
        }
        j += 1;
        while j < lines.len() && lines[j].trim() != "}," && lines[j].trim() != ");" {
            let a = lines[j].trim();
            if let Some((name, rest)) = a.split_once(':') {
                if rest.contains(" = (")
                    && name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
                {
                    args.push(Setter {
                        name: name.to_string(),
                        doc: doc_above(&lines, j),
                        rust: None,
                    });
                }
            }
            j += 1;
        }
        out.push((builder.to_string(), rname.to_string(), args));
    }
    out
}

/// Hand-written setters (`pub fn x(mut self, ..)`) in `impl .. Builder`
/// blocks, e.g. `ParCall`'s page layout and `LayoutCall`.
fn manual_setters(src: &[(PathBuf, String)], builder: &str) -> Vec<Setter> {
    let mut out = Vec::new();
    for (_, text) in src {
        let lines: Vec<&str> = text.lines().collect();
        let mut inside = false;
        for (i, l) in lines.iter().enumerate() {
            if l.starts_with("impl") {
                inside = l.contains(&format!(" {builder}<"));
            } else if l.starts_with('}') {
                inside = false;
            }
            let t = l.trim();
            if inside {
                if let Some(rest) = t.strip_prefix("pub fn ") {
                    let name: String = rest
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    let is_setter = rest[name.len()..].starts_with("(mut self")
                        || rest[name.len()..].starts_with("<");
                    if is_setter
                        && !matches!(
                            name.as_str(),
                            "with" | "draw" | "try_draw" | "get" | "try_get"
                        )
                    {
                        out.push(Setter {
                            name,
                            doc: doc_above(&lines, i),
                            rust: None,
                        });
                    }
                }
            }
        }
    }
    out
}

/// R's arguments per function (and `par`'s names), from the snapshot.
fn r_formals() -> BTreeMap<String, Vec<String>> {
    let text = fs::read_to_string(root().join("tests/data/r_formals.txt")).unwrap();
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let (k, v) = l.split_once(':').unwrap();
            (
                k.to_string(),
                v.split_whitespace().map(String::from).collect(),
            )
        })
        .collect()
}

/// Markdown links and intra-doc links reduced to their text, so the doc
/// reads the same anywhere.
fn plain(doc: &str) -> String {
    let mut s = String::new();
    let mut rest = doc;
    while let Some(start) = rest.find('[') {
        s.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        // A code span's `]` (`[`mar([5, 4])`]`) isn't the link's end.
        let close = if after.starts_with('`') {
            after.find("`]").map(|e| e + 1)
        } else {
            after.find(']')
        };
        match close {
            // A link is `[`X`]` (intra-doc) or `[text](url)`; other
            // brackets (`[0, 2, 5]`) stay.
            Some(end) if after.starts_with('`') || after[end + 1..].starts_with('(') => {
                let text = &after[..end];
                let tail = &after[end + 1..];
                s.push_str(text);
                rest = match tail.strip_prefix('(') {
                    Some(t) => t.find(')').map_or(tail, |e| &t[e + 1..]),
                    None => tail,
                };
            }
            Some(_) => {
                s.push('[');
                rest = after;
            }
            None => {
                s.push('[');
                rest = after;
            }
        }
    }
    s.push_str(rest);
    s
}

/// The first sentence of a doc.
fn summary(doc: &str) -> String {
    let doc = plain(doc);
    let mut end = doc.len();
    let b = doc.as_bytes();
    for i in 0..b.len() {
        if b[i] == b'.'
            && (i + 1 == b.len() || b[i + 1] == b' ')
            && !doc[..i].ends_with("e.g")
            && !doc[..i].ends_with("i.e")
        {
            // Not inside backticks.
            if doc[..i].matches('`').count() % 2 == 0 {
                end = i + 1;
                break;
            }
        }
    }
    doc[..end].replace('|', "\\|")
}

/// R's names a setter stands for: those its doc names ("R's `x`") that
/// are among `allowed`, else its own name with `_` as `.`, if allowed.
fn r_names(s: &Setter, allowed: &BTreeSet<String>) -> Vec<String> {
    let mut named = Vec::new();
    let mut rest = s.doc.as_str();
    while let Some(p) = rest.find("R's `") {
        let mut t = &rest[p + 5..];
        while let Some(e) = t.find('`') {
            let name = &t[..e];
            if allowed.contains(name) && !named.iter().any(|n| n == name) {
                named.push(name.to_string());
            }
            t = &t[e + 1..];
            match t.strip_prefix(" and `").or_else(|| t.strip_prefix(", `")) {
                Some(more) => t = more,
                None => break,
            }
        }
        rest = t;
    }
    if !named.is_empty() {
        return named;
    }
    let own = s.name.trim_end_matches('_').replace('_', ".");
    if allowed.contains(&own) {
        vec![own]
    } else {
        Vec::new()
    }
}

/// Setters standing for R arguments their names don't give: (R function,
/// setter) -> how R writes it. Checked to name existing setters.
const SPECIAL: &[(&str, &str, &str)] = &[
    ("barplot()", "legend_pos", "`args.legend` (`x`, `y`)"),
    ("text()", "vadj", "`adj` (second value)"),
    ("legend()", "vadj", "`adj` (second value)"),
    ("legend()", "pos", "`x`, `y`"),
    ("legend()", "labels", "`legend`"),
    ("axis()", "draw_labels", "`labels = FALSE`"),
    ("axis()", "axp", "`xaxp`, `yaxp`"),
    ("axis()", "axt", "`xaxt`, `yaxt`"),
    ("par()", "palette", "`palette()` (a function in R)"),
];

/// What to use instead of an R argument that isn't available.
const INSTEAD: &[(&str, &str, &str)] = &[
    ("plot()", "ann", "`dev.par().ann(false)`"),
    ("hist()", "probability", "`.freq(false)`"),
    ("hist()", "nclass", "`.breaks(n)`"),
    ("boxplot()", "plot", "`boxplot_stats()`"),
    ("abline()", "coef", "`.a()` and `.b()`"),
    (
        "abline()",
        "reg",
        "`.a()` and `.b()` with the model's coefficients",
    ),
];

/// Arguments R's plotting functions take through `...` besides `par()`'s.
const DOTS: &[&str] = &[
    "main", "sub", "xlab", "ylab", "xlim", "ylim", "axes", "gap.axis",
];

/// `bxp()`'s `pars` list elements.
const BXP_PARS: &[&str] = &[
    "boxwex",
    "staplewex",
    "outwex",
    "boxlty",
    "boxlwd",
    "boxcol",
    "boxfill",
    "medlty",
    "medlwd",
    "medpch",
    "medcex",
    "medcol",
    "medbg",
    "whisklty",
    "whisklwd",
    "whiskcol",
    "staplelty",
    "staplelwd",
    "staplecol",
    "outlty",
    "outlwd",
    "outpch",
    "outcex",
    "outcol",
    "outbg",
];

/// One function's section of the table.
struct Section {
    title: String,
    rust: String,
    setters: Vec<Setter>,
    /// R's arguments (`None`: no snapshot entry).
    formals: Vec<String>,
    /// Graphical parameters count as R arguments (R's `...` or `par()`).
    pars: bool,
}

fn render(sections: &[Section], par_names: &[String]) -> String {
    let readonly = ["cin", "cra", "csi", "cxy", "din", "page"];
    let mut out = String::new();
    out.push_str(
        "<!-- Generated by tests/all/generated_docs.rs from the code and \
         tests/data/r_formals.txt; don't edit. -->\n\n",
    );
    for sec in sections {
        let mut allowed: BTreeSet<String> = sec.formals.iter().cloned().collect();
        if sec.pars {
            allowed.extend(
                par_names
                    .iter()
                    .filter(|p| !readonly.contains(&p.as_str()))
                    .cloned(),
            );
            allowed.extend(DOTS.iter().map(|d| d.to_string()));
        }
        allowed.remove("...");
        // Outside `par()`, the `xlog`/`ylog` setters are R's `log`.
        if sec.title != "par()" {
            allowed.remove("xlog");
            allowed.remove("ylog");
        }
        out.push_str(&format!("### `{}`\n\n`{}`\n\n", sec.title, sec.rust));
        // R's arguments in R's order, then graphical parameters, then the
        // setters with no R argument.
        let mut rows: Vec<(usize, String, String, String)> = Vec::new();
        let mut used = BTreeSet::new();
        for s in &sec.setters {
            let special = SPECIAL
                .iter()
                .find(|(f, n, _)| *f == sec.title && *n == s.name)
                .map(|(_, _, r)| r.to_string())
                .or_else(|| {
                    let has = |f: &str| sec.formals.iter().any(|a| a == f);
                    match s.name.as_str() {
                        "xlog" if has("log") => Some("`log = \"x\"`".into()),
                        "ylog" if has("log") => Some("`log = \"y\"`".into()),
                        n if has("pars") && BXP_PARS.contains(&n) => {
                            Some(format!("`pars = list({n} = )`"))
                        }
                        _ => None,
                    }
                });
            if let Some(r) = special {
                // Count the R arguments it names as available.
                for f in &sec.formals {
                    if r.contains(&format!("`{f}`"))
                        || r.contains(&format!("`{f} ="))
                        || r.contains(&format!("`{f}("))
                    {
                        used.insert(f.clone());
                    }
                }
                let rust = s.rust.clone().unwrap_or_else(|| format!("`.{}()`", s.name));
                rows.push((usize::MAX - 2, r, rust, summary(&s.doc)));
                continue;
            }
            let names = r_names(s, &allowed);
            let key = names
                .first()
                .and_then(|n| sec.formals.iter().position(|f| f == n))
                .unwrap_or(if names.is_empty() {
                    usize::MAX
                } else {
                    usize::MAX - 1
                });
            let r = if names.is_empty() {
                "(none)".to_string()
            } else {
                names
                    .iter()
                    .map(|n| format!("`{n}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            used.extend(names);
            let rust = s.rust.clone().unwrap_or_else(|| format!("`.{}()`", s.name));
            rows.push((key, r, rust, summary(&s.doc)));
        }
        rows.sort_by_key(|r| r.0);
        out.push_str("| R | Rust | |\n|---|---|---|\n");
        for (_, r, rust, what) in &rows {
            out.push_str(&format!("| {r} | {rust} | {what} |\n"));
        }
        let missing: Vec<&String> = sec
            .formals
            .iter()
            .filter(|f| *f != "..." && !used.contains(*f))
            .filter(|f| !(sec.title == "par()" && readonly.contains(&f.as_str())))
            .collect();
        if !missing.is_empty() {
            out.push_str("\nR's arguments not available: ");
            let items: Vec<String> = missing
                .iter()
                .map(
                    |f| match INSTEAD.iter().find(|(s, a, _)| *s == sec.title && a == f) {
                        Some((_, _, use_)) => format!("`{f}` (use {use_})"),
                        None => format!("`{f}`"),
                    },
                )
                .collect();
            out.push_str(&items.join(", "));
            out.push_str(".\n");
        }
        out.push('\n');
    }
    out
}

fn sections() -> (Vec<Section>, Vec<String>) {
    let src = sources();
    let graphics = &src
        .iter()
        .find(|(p, _)| p.ends_with("src/graphics.rs"))
        .unwrap()
        .1;
    let formals = r_formals();
    let structs = options_structs(&src);
    let fields_of = |builder: &str| -> Vec<Setter> {
        structs
            .iter()
            .filter(|(_, calls, _)| calls.iter().any(|c| c == builder))
            .flat_map(|(_, _, f)| f.clone())
            .collect()
    };
    let par_fields: Vec<Setter> = structs
        .iter()
        .filter(|(n, _, _)| n == "Par")
        .flat_map(|(_, _, f)| f.clone())
        .collect();
    let mut sections = Vec::new();
    // `Graphics` methods, in the trait's order.
    let mut methods: Vec<(String, String)> = Vec::new();
    for l in graphics.lines() {
        let t = l.trim();
        if let Some(rest) = t.strip_prefix("fn ") {
            if let Some((name, ret)) = rest.split_once("(&mut self) -> ") {
                if let Some(b) = ret.strip_suffix("<'_, Self> {") {
                    methods.push((name.to_string(), b.to_string()));
                }
            }
        }
    }
    let calls = calls(graphics);
    for (method, builder) in &methods {
        // Hand-written builders (`layout()`) have their own section below.
        let Some((_, rname, args)) = calls.iter().find(|(b, _, _)| b == builder) else {
            continue;
        };
        let mut setters = args.clone();
        setters.extend(fields_of(builder));
        if builder == "ParCall" {
            setters.extend(par_fields.clone());
        }
        for m in manual_setters(&src, builder) {
            if !setters.iter().any(|s| s.name == m.name) {
                setters.push(m);
            }
        }
        let f = formals.get(rname.as_str()).cloned().unwrap_or_default();
        let pars = rname == "par" || f.iter().any(|a| a == "...");
        let formals = if rname == "par" {
            formals["par"].clone()
        } else {
            f
        };
        sections.push(Section {
            title: format!("{rname}()"),
            rust: format!("dev.{method}()"),
            setters,
            formals,
            pars,
        });
    }
    // Device methods and devices.
    sections.push(Section {
        title: "plot.new()".into(),
        rust: "dev.plot_new()".into(),
        setters: Vec::new(),
        formals: formals["plot.new"].clone(),
        pars: false,
    });
    sections.push(Section {
        title: "layout()".into(),
        rust: "dev.layout()".into(),
        setters: manual_setters(&src, "LayoutCall"),
        formals: formals["layout"].clone(),
        pars: false,
    });
    for (r, rust) in [
        ("strwidth", "dev.strwidth()"),
        ("strheight", "dev.strheight()"),
    ] {
        sections.push(Section {
            title: format!("{r}()"),
            rust: rust.into(),
            setters: manual_setters(&src, "StrSizeCall"),
            formals: formals[r].clone(),
            pars: false,
        });
    }
    let device_fields: Vec<Setter> = structs
        .iter()
        .filter(|(n, _, _)| n == "DeviceBuilder")
        .flat_map(|(_, _, f)| f.clone())
        .collect();
    for dev in ["pdf", "svg", "png"] {
        let mut setters = vec![Setter {
            name: "file".into(),
            doc: if dev == "pdf" {
                "The file to write (with a page number format, a file per page); \
                 `Device::null()` for R's `pdf(NULL)`. R's `file`."
                    .into()
            } else {
                "The file to write (with a page number format, a file per page). \
                 R's `file`, `filename`."
                    .into()
            },
            rust: Some(format!("`Device::{dev}(file)`")),
        }];
        setters.extend(device_fields.clone());
        sections.push(Section {
            title: format!("{dev}()"),
            rust: format!("Device::{dev}(file)"),
            setters,
            formals: formals[dev].clone(),
            pars: false,
        });
    }
    (sections, formals["par"].clone())
}

#[test]
fn special_mappings_name_existing_setters() {
    let (sections, _) = sections();
    for (f, n, _) in SPECIAL {
        let sec = sections
            .iter()
            .find(|s| s.title == *f)
            .unwrap_or_else(|| panic!("{f}"));
        assert!(
            sec.setters.iter().any(|s| s.name == *n),
            "{f}: no setter {n}"
        );
    }
    for (f, a, _) in INSTEAD {
        let sec = sections
            .iter()
            .find(|s| s.title == *f)
            .unwrap_or_else(|| panic!("{f}"));
        assert!(
            sec.formals.iter().any(|x| x == a),
            "{f}: R has no argument {a}"
        );
    }
}

#[test]
fn r_reference_table_is_up_to_date() {
    let (sections, par) = sections();
    check_generated("src/guide/r_reference.md", &render(&sections, &par));
}

/// A chart drawn as SVG for embedding in a doc comment: no XML declaration,
/// scaled to the page width, and clip ids prefixed with `name` so several
/// charts can share a page.
fn chart_svg(
    name: &str,
    (w, h): (f64, f64),
    draw: fn(&mut Device<AnyBackend>) -> Result<(), rs_graphics::Error>,
) -> String {
    let mut dev = Device::memory("svg").width(w).height(h).open().unwrap();
    draw(&mut dev).unwrap();
    let svg = String::from_utf8(dev.finish()).unwrap();
    let start = svg.find("<svg").unwrap();
    let svg = &svg[start..];
    let open_end = svg.find('>').unwrap();
    let rest = &svg[open_end..];
    let view_box = {
        let v = svg[..open_end].split("viewBox=\"").nth(1).unwrap();
        &v[..v.find('"').unwrap()]
    };
    let out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{view_box}\" \
         role=\"img\" aria-label=\"{name} chart\" \
         style=\"width: 100%; max-width: {}px; height: auto; background: white\"{rest}",
        (w * 96.0) as u32
    );
    let out = out
        .replace("id=\"clip", &format!("id=\"{name}-clip"))
        .replace("url(#clip", &format!("url(#{name}-clip"));
    // A blank line would end the HTML block in the doc's markdown.
    assert!(!out.contains("\n\n"), "{name}: blank line in the SVG");
    out
}

#[test]
fn charts_are_up_to_date() {
    type Chart = fn(&mut Device<AnyBackend>) -> Result<(), rs_graphics::Error>;
    let charts: [(&str, (f64, f64), Chart); 7] = [
        ("pch", (8.0, 4.5), charts::pch),
        ("line_types", (7.0, 5.0), charts::line_types),
        ("line_ends", (7.0, 4.0), charts::line_ends),
        ("fonts", (7.0, 4.0), charts::fonts),
        ("colors", (9.0, 12.0), charts::colors),
        ("palette", (7.0, 3.0), charts::palette),
        ("regions", (7.0, 6.0), charts::regions),
    ];
    for (name, size, draw) in charts {
        check_generated(
            &format!("src/charts/{name}.svg"),
            &chart_svg(name, size, draw),
        );
    }
}

/// A guide page's markdown as plain text for `llms.txt`: headings
/// `shift` levels down, Rust code blocks without their hidden (`# `)
/// lines, links reduced to their text.
fn for_llms(md: &str, shift: &str) -> String {
    let mut out = String::new();
    let mut in_rust = false;
    let mut in_code = false;
    for line in md.lines() {
        if let Some(info) = line.strip_prefix("```") {
            if in_code {
                in_code = false;
                in_rust = false;
                out.push_str("```\n");
            } else {
                in_code = true;
                in_rust = info.is_empty() || info == "no_run" || info == "rust";
                out.push_str(if in_rust { "```rust\n" } else { line });
                if !in_rust {
                    out.push('\n');
                }
            }
            continue;
        }
        if in_code {
            if in_rust && (line == "#" || line.starts_with("# ")) {
                continue;
            }
            out.push_str(line);
        } else if line.starts_with("[`") && line.contains("`]: crate::") {
            // A rustdoc link definition: the link text stays, plainly.
            continue;
        } else if line.starts_with('#') {
            out.push_str(shift);
            out.push_str(line);
        } else {
            out.push_str(&plain(line));
        }
        out.push('\n');
    }
    out
}

#[test]
fn llms_txt_is_up_to_date() {
    let read = |rel: &str| fs::read_to_string(root().join(rel)).unwrap();
    let (sections, par) = sections();
    let mut out = String::from(
        "# rs-graphics\n\n\
         > A Rust port of R's `graphics` package: the same functions and \
         arguments, drawing to PDF, SVG and PNG with R's layout. Every R \
         function is a method of a device (`dev.plot()`), every R argument \
         a setter named as in R (`.cex_main()` for `cex.main`), and \
         `.draw()` runs the call.\n\n\
         Generated from the crate's guides (`rs_graphics::guide`) and code; \
         the API docs are built with `cargo doc --open`.\n\n\
         ```rust\n\
         use rs_graphics::prelude::*;\n\n\
         fn main() -> Result<(), Box<dyn std::error::Error>> {\n    \
         let mut dev = Device::pdf(\"plot.pdf\").open()?;\n    \
         dev.plot().x([1, 2, 3]).y([2, 4, 3]).pch(19).main(\"Title\").draw();\n    \
         dev.close()?;\n    \
         Ok(())\n\
         }\n\
         ```\n\n",
    );
    out.push_str("## Coming from R\n\n");
    out.push_str(&for_llms(&read("src/guide/coming_from_r.md"), "##"));
    out.push_str("\n## Concepts\n\n");
    out.push_str(&for_llms(&read("src/guide/concepts.md"), "##"));
    out.push_str("\n## Every R function and argument\n\n");
    out.push_str(&for_llms(&render(&sections, &par), ""));
    check_generated("llms.txt", &out);
}

/// Every `par(x)` in a doc comment links to what it names (written
/// `` [`par(cex)`] `` with a `` [`par(cex)`]: crate::par::Par::cex ``
/// definition at the end of the doc), except in code blocks, a `Par`
/// field's mention of itself, and `par(family)`, which isn't supported.
#[test]
fn par_mentions_are_links() {
    let mut bare = Vec::new();
    for (path, text) in sources() {
        let mut in_code = false;
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim_start();
            let Some(doc) = t.strip_prefix("///").or_else(|| t.strip_prefix("//!")) else {
                continue;
            };
            if doc.trim_start().starts_with("```") {
                in_code = !in_code;
                continue;
            }
            if in_code {
                continue;
            }
            let mut rest = doc;
            while let Some(p) = rest.find("`par(") {
                let after = &rest[p + 5..];
                let end = after.find(")`").map_or(after.len(), |e| e);
                let name = after[..end].trim_matches('"');
                let linked = rest[..p].ends_with('[') && after[end..].starts_with(")`]");
                let is_name =
                    !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '.');
                // A `Par` field's own doc doesn't link to itself.
                let own_field = path.ends_with("src/par.rs")
                    && lines[i..]
                        .iter()
                        .find(|l| !l.trim_start().starts_with("///"))
                        .and_then(|l| l.trim_start().strip_prefix("pub "))
                        .and_then(|l| l.split_once(':'))
                        .is_some_and(|(f, _)| f == name.replace('.', "_"));
                if is_name && !linked && name != "family" && !own_field {
                    bare.push(format!("{}:{}: `par({name})`", path.display(), i + 1));
                }
                rest = &after[end..];
            }
        }
    }
    assert!(
        bare.is_empty(),
        "unlinked par() mentions:\n{}",
        bare.join("\n")
    );
}
