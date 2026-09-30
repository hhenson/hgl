//! Module parts, imports, declarations and test-scope indexing for HGL.
use hgl_source::{Cursor, Expr, Token, lex};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
/// The declaration role before overload selection.
pub enum Role {
    /// Ordinary or const function.
    Function,
    /// Operator implementation.
    Implementation,
    /// Operator contract.
    Operator,
    /// Native interface or selected target part.
    Native,
    /// Named test case.
    Test,
}
#[derive(Debug, Clone)]
/// A declaration with its owning module and source tokens.
pub struct Decl {
    /// Owning module name.
    pub module: String,
    /// Source identifier.
    /// Source identifier.
    pub name: String,
    /// Declaration category.
    pub role: Role,
    /// Retained source tokens.
    pub tokens: Vec<Token>,
    /// Diagnostic source name.
    pub source: String,
    /// Source documentation.
    pub doc: String,
    /// Visible only during this module's test construction.
    pub test_only: bool,
}
#[derive(Debug, Clone)]
/// One parameter in an uninstantiated signature.
pub struct Parameter {
    /// Source identifier.
    /// Source identifier.
    pub name: String,
    /// Source type expression.
    pub ty: String,
    /// Wiring-time parameter.
    pub constant: bool,
    /// Optional fixed default expression.
    pub default: Option<Expr>,
}
#[derive(Debug, Clone)]
/// A callable signature and its retained body.
pub struct Signature {
    /// Generic parameter identifiers.
    pub generics: Vec<String>,
    /// Ordered source parameters.
    pub parameters: Vec<Parameter>,
    /// Source result type expression.
    pub result: String,
    /// Body tokens, empty for an interface.
    pub body: Vec<Token>,
    /// Const function, eligible for temporal lifting.
    pub value_function: bool,
    /// Declared native error channel.
    pub throws: bool,
    /// Finite type-domain constraint, checked after generic inference.
    pub type_domain: Option<(String, Vec<String>)>,
    /// Native scalar requirement: name, arguments and result.
    pub requirement: Option<(String, Vec<String>, String)>,
}
#[derive(Debug, Default, Clone)]
/// Indexed source declarations, imports and explicit instances.
pub struct Library {
    /// All indexed declarations.
    pub declarations: Vec<Decl>,
    /// First source's module.
    pub root: String,
    /// Module and alias to qualified target.
    pub imports: BTreeMap<(String, String), String>,
    /// Explicit generic operator instances.
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

/// Index sources and validate module-part and test-context boundaries.
pub fn load(sources: &[(String, String)]) -> Result<Library, String> {
    let mut library = Library::default();
    let mut parts = BTreeSet::new();
    for (source, text) in sources {
        load_source(&mut library, &mut parts, source, text, false)
            .map_err(|e| format!("{source}: {e}"))?;
    }
    Ok(library)
}
fn load_source(
    library: &mut Library,
    parts: &mut BTreeSet<(String, String)>,
    source: &str,
    text: &str,
    test_context: bool,
) -> Result<(), String> {
    let tokens = lex(text).map_err(|e| format!("{source}: {e}"))?;
    let mut c = Cursor::new(&tokens);
    c.lines();
    let mut doc = String::new();
    if c.peek().starts_with("/**") {
        doc = c.consume()?;
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
            doc = c.consume()?;
            c.lines();
            continue;
        }
        let start = c.pos;
        let mut depth = 0_i32;
        while !c.at("") {
            let t = c.consume()?;
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
        if test_context
            && (!matches!(d.peek(), "fn" | "const" | "test")
                || (d.at("test") && chunk.get(1).is_some_and(|t| t.text == "{")))
        {
            return Err(
                "test context admits only private fn/const fn helpers and named tests".into(),
            );
        }
        if d.take("test") && d.take("{") {
            let end = chunk
                .iter()
                .rposition(|t| t.text == "}")
                .ok_or("unclosed test context")?;
            let body = &text[chunk[2].span.start..chunk[end].span.start];
            let nested = format!("module {module} part __test_{}\n{body}", parts.len());
            let first = library.declarations.len();
            load_source(library, parts, source, &nested, true)?;
            for decl in &mut library.declarations[first..] {
                decl.test_only = true;
            }
            continue;
        }
        d.pos = 0;
        if d.take("use") {
            imports(library, &module, &mut d)?;
        } else if d.take("instantiate") {
            instantiate(library, &module, &mut d)?;
        } else {
            d.take("export");
            let role = if d.take("test") {
                Role::Test
            } else if d.take("operator") {
                Role::Operator
            } else if d.take("impl") {
                d.need("fn")?;
                Role::Implementation
            } else if d.take("native") {
                d.take("const");
                d.need("fn")?;
                Role::Native
            } else if d.take("const") {
                d.need("fn")?;
                Role::Function
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
                test_only: false,
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
    /// Parse this declaration as a callable signature.
    pub fn signature(&self) -> Result<Signature, String> {
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
        let value_function = c.take("const");
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
            let ty = c.type_name()?;
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
            c.type_name()?
        } else {
            "void".into()
        };
        let throws = c.take("throws");
        c.lines();
        let mut type_domain = None;
        let requires = c.take("requires");
        let requirement = if requires && c.tokens.get(c.pos + 1).is_some_and(|t| t.text == "in") {
            type_domain = Some(parse_domain(&mut c, &generics)?);
            None
        } else if requires {
            Some(parse_native_requirement(&mut c)?)
        } else {
            None
        };
        c.lines();
        Ok(Signature {
            generics,
            value_function,
            throws,
            requirement,
            type_domain,
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
            types.push(d.consume()?);
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

fn parse_domain(c: &mut Cursor<'_>, generics: &[String]) -> Result<(String, Vec<String>), String> {
    let parameter = c.name()?;
    if !generics.contains(&parameter) {
        return Err("type domain requires a generic parameter".into());
    }
    c.need("in")?;
    c.need("{")?;
    let mut types = Vec::new();
    loop {
        c.lines();
        let ty = c.type_name()?;
        if hgl_source::Ty::parse(&ty).is_none() || types.contains(&ty) {
            return Err("type domain requires distinct supported types".into());
        }
        types.push(ty);
        c.lines();
        if !c.take(",") {
            c.need("}")?;
            break;
        }
    }
    Ok((parameter, types))
}

fn parse_native_requirement(c: &mut Cursor<'_>) -> Result<(String, Vec<String>, String), String> {
    let Expr::Call(name, args) = c.expr()? else {
        return Err("expected native requirement".into());
    };
    let args = args
        .into_iter()
        .map(|(_, e)| {
            if let Expr::Name(n) = e {
                Ok(n)
            } else {
                Err("requirement takes types".to_owned())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    c.need("->")?;
    Ok((name, args, c.name()?))
}
