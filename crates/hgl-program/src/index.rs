use crate::syntax::{Cursor, Expr, Token, lex};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Role {
    Function,
    Implementation,
    Operator,
    Native,
}
#[derive(Debug, Clone)]
pub(crate) struct Decl {
    pub module: String,
    pub name: String,
    pub role: Role,
    pub tokens: Vec<Token>,
    pub source: String,
    pub doc: String,
}
#[derive(Debug, Clone)]
pub(crate) struct Parameter {
    pub name: String,
    pub ty: String,
    pub constant: bool,
    pub default: Option<Expr>,
}
#[derive(Debug, Clone)]
pub(crate) struct Signature {
    pub generics: Vec<String>,
    pub parameters: Vec<Parameter>,
    pub result: String,
    pub body: Vec<Token>,
}
#[derive(Debug, Default)]
pub(crate) struct Library {
    pub declarations: Vec<Decl>,
    pub root: String,
    pub imports: BTreeMap<(String, String), String>,
    pub instances: BTreeSet<(String, String, Vec<String>)>,
}

fn boundary(s: &str) -> bool {
    matches!(
        s,
        "module"
            | "use"
            | "operator"
            | "impl"
            | "export"
            | "native"
            | "fn"
            | "struct"
            | "abstract"
            | "instantiate"
            | "test"
    ) || s.starts_with("/**")
}

pub(crate) fn load(sources: &[(String, String)]) -> Result<Library, String> {
    let mut library = Library::default();
    let mut parts = BTreeSet::new();
    for (source, text) in sources {
        load_source(&mut library, &mut parts, source, text)
            .map_err(|e| format!("{source}: {e}"))?;
    }
    Ok(library)
}
fn load_source(
    library: &mut Library,
    parts: &mut BTreeSet<(String, String)>,
    source: &str,
    text: &str,
) -> Result<(), String> {
    let tokens = lex(text).map_err(|e| format!("{source}: {e}"))?;
    let mut c = Cursor::new(&tokens);
    c.lines();
    let mut doc = String::new();
    if c.peek().starts_with("/**") {
        doc = c.next()?;
        c.lines();
    }
    c.need("module")?;
    let mut module = c.name()?;
    while c.take(".") {
        module.push('.');
        module.push_str(&c.name()?);
    }
    if library.root.is_empty() {
        library.root.clone_from(&module);
    }
    let part = if c.take("part") {
        c.name()?
    } else {
        String::new()
    };
    if !parts.insert((module.clone(), part)) {
        return Err(format!("{source}: duplicate module part"));
    }
    if !c.at("\n") && !c.at("") {
        return Err(format!("{source}: invalid module header"));
    }
    c.lines();
    // Module documentation is not a declaration's documentation.
    doc.clear();
    while !c.at("") {
        if c.peek().starts_with("/**") {
            doc = c.next()?;
            c.lines();
            continue;
        }
        let start = c.pos;
        let mut depth = 0_i32;
        while !c.at("") {
            let t = c.next()?;
            if matches!(t.as_str(), "(" | "{" | "[") {
                depth += 1;
            }
            if matches!(t.as_str(), ")" | "}" | "]") {
                depth -= 1;
            }
            if depth < 0 {
                return Err(format!("{source}: unmatched closing delimiter"));
            }
            if depth == 0 && t == "\n" {
                c.lines();
                if boundary(c.peek())
                    || (c.at("const") && c.tokens.get(c.pos + 1).is_some_and(|t| t.text == "fn"))
                    || c.at("")
                {
                    break;
                }
            }
        }
        if depth != 0 {
            return Err(format!("{source}: unclosed declaration"));
        }
        let chunk = &tokens[start..c.pos];
        let mut d = Cursor::new(chunk);
        if d.take("use") {
            imports(library, &module, &mut d)?;
        } else if d.take("instantiate") {
            instantiate(library, &module, &mut d)?;
        } else {
            d.take("export");
            let role = if d.take("operator") {
                Role::Operator
            } else if d.take("impl") {
                d.need("fn")?;
                Role::Implementation
            } else if d.take("native") {
                d.take("const");
                d.need("fn")?;
                Role::Native
            } else if d.take("fn") {
                Role::Function
            } else {
                doc.clear();
                continue;
            };
            let name = d.name()?;
            library.declarations.push(Decl {
                module: module.clone(),
                name,
                role,
                tokens: chunk.to_vec(),
                source: source.to_owned(),
                doc: std::mem::take(&mut doc),
            });
        }
        doc.clear();
    }

    Ok(())
}

fn add_import(
    library: &mut Library,
    module: &str,
    alias: &str,
    target: &str,
) -> Result<(), String> {
    if let Some(old) = library
        .imports
        .insert((module.into(), alias.to_owned()), target.to_owned())
        && old != target
    {
        return Err(format!("conflicting import {alias} in {module}"));
    }
    Ok(())
}
impl Decl {
    pub(crate) fn signature(&self) -> Result<Signature, String> {
        self.parse_signature().map_err(|e| {
            format!(
                "{}:{}: {}::{}: {e}",
                self.source,
                self.tokens.first().map_or(0, |t| t.span.start),
                self.module,
                self.name
            )
        })
    }
    fn parse_signature(&self) -> Result<Signature, String> {
        let mut c = Cursor::new(&self.tokens);
        c.take("export");
        c.take("impl");
        if c.take("native") {
            c.need("const")?;
        }
        if !c.take("fn") {
            c.need("operator")?;
        }
        c.name()?;
        let mut generics = Vec::new();
        if c.take("<") {
            loop {
                generics.push(c.name()?);
                if !c.take(",") {
                    c.need(">")?;
                    break;
                }
            }
        }
        c.need("(")?;
        c.lines();
        let mut parameters = Vec::new();
        while !c.take(")") {
            let constant = c.take("const");
            let name = c.name()?;
            c.need(":")?;
            let ty = c.name()?;
            let default = if c.take("=") { Some(c.expr()?) } else { None };
            parameters.push(Parameter {
                name,
                ty,
                constant,
                default,
            });
            c.lines();
            if !c.take(",") {
                c.need(")")?;
                break;
            }
            c.lines();
        }
        let result = if c.take("->") {
            c.name()?
        } else {
            "void".into()
        };
        c.lines();
        Ok(Signature {
            generics,
            parameters,
            result,
            body: c.tokens[c.pos..].to_vec(),
        })
    }
}

fn imports(library: &mut Library, module: &str, d: &mut Cursor<'_>) -> Result<(), String> {
    let mut target = d.name()?;
    while d.take(".") {
        target.push('.');
        target.push_str(&d.name()?);
    }
    if d.take("as") {
        let alias = d.name()?;
        add_import(library, module, &alias, &target)?;
    } else {
        d.need("::")?;
        d.need("{")?;
        d.lines();
        while !d.take("}") {
            let name = d.name()?;
            add_import(library, module, &name, &format!("{target}::{name}"))?;
            d.lines();
            if !d.take(",") {
                d.need("}")?;
                break;
            }
            d.lines();
        }
    }

    Ok(())
}

fn instantiate(library: &mut Library, module: &str, d: &mut Cursor<'_>) -> Result<(), String> {
    loop {
        d.lines();
        let name = d.name()?;
        d.need("<")?;
        let mut types = Vec::new();
        while !d.take(">") {
            types.push(d.next()?);
            if !d.take(",") {
                d.need(">")?;
                break;
            }
        }
        library.instances.insert((module.to_owned(), name, types));
        if !d.take(",") {
            break;
        }
    }

    Ok(())
}
