//! Module parts, imports, declarations and test-scope indexing for HGL.
use hgl_source::diagnostics::Diagnostic;
use hgl_source::{Cursor, Expr, Issue, Token, lex};
use std::collections::{BTreeMap, BTreeSet};

pub use hgl_source::DeclarationRole as Role;
#[derive(Debug, Clone)]
/// A declaration with its owning module and source tokens.
pub struct Decl {
    /// Ordinary value mode established only by a validated package contract.
    pub ordinary_native: bool,
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
    /// Original tokens of the annotation, retaining primary diagnostic locations.
    pub type_tokens: Vec<Token>,
    /// Wiring-time parameter.
    pub constant: bool,
    /// Optional required ordinary default expression.
    pub default: Option<Expr>,
    /// Checked independently owned required default, materialized before binding.
    pub value_default: Option<crate::ir::Value>,
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
    /// Original result annotation tokens.
    pub result_tokens: Vec<Token>,
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
pub use crate::struct_declarations::RequiredStruct;
#[derive(Debug, Default, Clone)]
/// Indexed source declarations, imports and explicit instances.
pub struct Library {
    /// Data-only selected native package contracts; never executable providers.
    pub native_contracts: crate::native_contracts::Contracts,
    /// Module whose test-local nominal descendants participate in type resolution.
    pub test_scope: Option<String>,
    /// Declaration-owned bound literals, retaining exact count or duration kind.
    pub type_sizes: BTreeMap<(String, String), hgl_source::Literal>,
    /// Shared cold source declarations; detach with `Arc::make_mut` before mutation.
    pub declarations: std::sync::Arc<[Decl]>,
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
            | "enum"
            | "abstract"
            | "instantiate"
            | "test"
    ) || s.starts_with("/**")
}

/// Index sources and validate module-part and test-context boundaries.
pub fn load(sources: &[(String, String)]) -> Result<Library, String> {
    load_checked(sources).map_err(|e| e.to_string())
}
/// Index declarations while retaining structured source-origin parser failures.
pub fn load_checked(sources: &[(String, String)]) -> Result<Library, Box<Diagnostic>> {
    let mut library = Library::default();
    let mut parts = BTreeSet::new();
    let mut declarations = Vec::new();
    for (source, text) in sources {
        let checked = if crate::native_contracts::sidecar(source) {
            crate::native_contracts::parse(source, text).and_then(|descriptor| {
                if library
                    .native_contracts
                    .insert(descriptor.module.clone(), descriptor)
                    .is_some()
                {
                    Err("multiple selected native descriptors for one module".into())
                } else {
                    Ok(())
                }
            })
        } else {
            load_source(&mut library, &mut declarations, &mut parts, source, text)
        };
        checked.map_err(|e| Box::new(Diagnostic::new(source, text, e)))?;
    }
    library.declarations = declarations.into();
    test_types(&library)
        .and_then(|()| crate::native_contracts::validate(&library))
        .map_err(|issue| {
            let (source, text) = sources
                .iter()
                .find(|(source, _)| issue.source.as_ref() == Some(source))
                .or_else(|| sources.first())
                .map_or(("", ""), |(source, text)| (source.as_str(), text.as_str()));
            Box::new(Diagnostic::new(source, text, issue))
        })?;
    for declaration in std::sync::Arc::make_mut(&mut library.declarations) {
        declaration.ordinary_native = declaration.role == Role::Native
            && library
                .native_contracts
                .get(&declaration.module)
                .is_some_and(|contract| contract.functions.contains_key(&declaration.name));
    }
    Ok(library)
}
fn test_types(library: &Library) -> Result<(), Issue> {
    for (index, decl) in library.declarations.iter().enumerate() {
        if !decl.test_only || !matches!(decl.role, Role::Struct | Role::Enum) {
            continue;
        }
        // Test-local nominal names must be free in the module (syntax-and-semantics
        // test contexts); helper names share constructor syntax, test labels do not.
        let conflict = library
            .imports
            .contains_key(&(decl.module.clone(), decl.name.clone()))
            || library
                .declarations
                .iter()
                .enumerate()
                .any(|(other_index, other)| {
                    other_index != index
                        && other.module == decl.module
                        && other.name == decl.name
                        && other.role != Role::Test
                });
        if conflict {
            return Err(Issue::from(format!(
                "conflicting test-local type {}::{}",
                decl.module, decl.name
            ))
            .at(decl.tokens[0].span.clone())
            .in_source(&decl.source));
        }
        let name = format!("{}::{}", decl.module, decl.name);
        if library.imports.values().any(|target| target == &name) {
            return Err(
                Issue::from(format!("test-only type {name} cannot be imported"))
                    .at(decl.tokens[0].span.clone())
                    .in_source(&decl.source),
            );
        }
    }
    Ok(())
}
fn load_source(
    library: &mut Library,
    declarations: &mut Vec<Decl>,
    parts: &mut BTreeSet<(String, String)>,
    source: &str,
    text: &str,
) -> Result<(), Issue> {
    let tokens = lex(text).map_err(|e| format!("{source}: {e}"))?;
    let mut c = Cursor::new(&tokens);
    c.lines();
    if c.peek().starts_with("/**") {
        c.consume()?;
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
        return Err(format!("{source}: duplicate module part").into());
    }
    if !c.at("\n") && !c.at("") {
        return Err(format!("{source}: invalid module header").into());
    }
    c.lines();
    load_items(
        library,
        declarations,
        &module,
        source,
        &tokens[c.pos..],
        false,
    )
}
fn load_items(
    library: &mut Library,
    declarations: &mut Vec<Decl>,
    module: &str,
    source: &str,
    tokens: &[Token],
    test_context: bool,
) -> Result<(), Issue> {
    let mut c = Cursor::new(tokens);
    c.lines();
    let mut doc = String::new();
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
                return Err(Issue::from("unmatched closing delimiter").at(c.span()));
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
        let chunk = &tokens[start..c.pos];
        let mut d = Cursor::new(chunk);
        if test_context
            && (!matches!(
                d.peek(),
                "fn" | "const" | "test" | "struct" | "enum" | "abstract"
            ) || (d.at("test") && chunk.get(1).is_some_and(|t| t.text == "{")))
        {
            return Err(
                "test context admits only private fn/const fn helpers, struct/enum types and named tests".into(),
            );
        }
        if d.take("test") && d.take("{") {
            if depth != 0 {
                return Err(Issue::coded(
                    "parse",
                    "syntax.expected_token",
                    c.span(),
                    "expected } to close test context",
                ));
            }
            let end = chunk
                .iter()
                .rposition(|t| t.text == "}")
                .ok_or("unclosed test context")?;
            load_items(library, declarations, module, source, &chunk[2..end], true)?;
            continue;
        }
        d.pos = 0;
        if d.take("use") {
            imports(library, module, &mut d)?;
        } else if d.take("instantiate") {
            instantiate(library, module, &mut d)?;
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
                if d.take("type") {
                    Role::NativeType
                } else {
                    d.take("const");
                    d.need("fn")?;
                    Role::Native
                }
            } else if d.take("const") {
                d.need("fn")?;
                Role::Function
            } else if d.take("enum") {
                Role::Enum
            } else if d.take("abstract") {
                d.need("struct")?;
                Role::Struct
            } else if d.take("struct") {
                Role::Struct
            } else if d.take("fn") {
                Role::Function
            } else {
                doc.clear();
                continue;
            };
            let name = d.name()?;
            declarations.push(Decl {
                ordinary_native: false,
                module: module.into(),
                name,
                role,
                tokens: chunk.to_vec(),
                source: source.to_owned(),
                doc: std::mem::take(&mut doc),
                test_only: test_context,
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
    /// Validate the complete opaque declaration without exposing representation.
    pub fn native_type(&self) -> Result<(), Issue> {
        let mut c = Cursor::new(&self.tokens);
        c.take("export");
        c.need("native")?;
        c.need("type")?;
        c.name()?;
        c.lines();
        c.need("")?;
        Ok(())
    }
    /// Parse declaration-owned fields, defaults and single inheritance.
    pub fn required_struct(&self) -> Result<RequiredStruct, String> {
        crate::struct_declarations::parse(&self.tokens)
    }

    /// Parse this declaration as a callable signature.
    pub fn signature(&self) -> Result<Signature, String> {
        self.signature_checked().map_err(|e| {
            format!(
                "{}:{}: {}::{}: {}",
                self.source,
                self.tokens.first().map_or(0, |t| t.span.start),
                self.module,
                self.name,
                e.message
            )
        })
    }
    /// Parse a signature without discarding diagnostic code or byte location.
    pub fn signature_checked(&self) -> Result<Signature, Issue> {
        let mut c = Cursor::new(&self.tokens);
        c.take("export");
        c.take("impl");
        if c.take("native") {
            c.need("const")?;
        }
        let value_function = c.take("const") || self.ordinary_native;
        if !c.take("fn") {
            c.need("operator")?;
        }
        c.name()?;
        let mut generics = Vec::new();
        if c.take("<") {
            loop {
                let constant = c.take("const");
                generics.push(c.name()?);
                if constant {
                    c.need(":")?;
                    c.type_name()?;
                }
                if !c.take(",") {
                    c.need(">")?;
                    break;
                }
            }
        }
        let parameters = parameters(&mut c, value_function)?;
        let mut result_tokens = Vec::new();
        let result = if c.take("->") {
            let start = c.pos;
            let name = c.type_name()?;
            result_tokens = c.tokens[start..c.pos].to_vec();
            name
        } else {
            "void".into()
        };
        if c.at("throws") {
            return Err(Issue::coded(
                "syntax",
                "syntax.expected_token",
                c.span(),
                "source throws clauses are not admitted; recoverable native functions return result<T,error>",
            ));
        }
        let throws = false;
        c.lines();
        let mut type_domain = None;
        let requires = c.take("requires");
        let requirement = if requires && c.tokens.get(c.pos + 1).is_some_and(|t| t.text == "in") {
            type_domain = Some(crate::struct_declarations::parse_domain(&mut c, &generics)?);
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
            result_tokens,
            body: c.tokens[c.pos..].to_vec(),
        })
    }
}

fn imports(library: &mut Library, module: &str, d: &mut Cursor<'_>) -> Result<(), Issue> {
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
            types.push(d.type_name()?);
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

fn parse_native_requirement(c: &mut Cursor<'_>) -> Result<(String, Vec<String>, String), String> {
    let Expr::Call(name, args) = c.expr()?.syntax().clone() else {
        return Err("expected native requirement".into());
    };
    let args = args
        .into_iter()
        .map(|(_, e)| {
            if let Expr::Name(n) = e.syntax() {
                Ok(n.clone())
            } else {
                Err("requirement takes types".to_owned())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    c.need("->")?;
    Ok((name, args, c.name()?))
}

fn parameters(c: &mut Cursor<'_>, value_function: bool) -> Result<Vec<Parameter>, Issue> {
    c.need("(")?;
    c.lines();
    let mut parameters = Vec::new();
    while !c.take(")") {
        let constant = c.take("const");
        let name = c.name()?;
        c.need(":")?;
        let type_start = c.pos;
        let pack = c.take(".");
        if pack {
            c.need(".")?;
            c.need(".")?;
        }
        let mut ty = c.type_name()?;
        if pack {
            ty = format!("...{ty}");
        }
        let type_tokens = c.tokens[type_start..c.pos].to_vec();
        if (constant || value_function) && !hgl_source::value_type(&ty) {
            return Err(
                "const parameter annotations require value_type; atomic is a temporal boundary"
                    .into(),
            );
        }
        let default = if c.take("=") { Some(c.expr()?) } else { None };
        parameters.push(Parameter {
            name,
            ty,
            type_tokens,
            constant,
            default,
            value_default: None,
        });
        c.lines();
        if !c.take(",") {
            c.need(")")?;
            break;
        }
        c.lines();
    }
    Ok(parameters)
}

/// Locate a relative normalized annotation error in its original source tokens.
pub fn locate_type_issue(mut issue: Issue, tokens: &[Token], source: &str) -> Issue {
    if issue.source.is_some() {
        return issue;
    }
    let mut offset = 0;
    let primary = tokens.iter().filter(|t| t.text != "\n").find(|token| {
        let includes = issue.span.start < offset + token.text.len();
        offset += token.text.len();
        includes
    });
    if let Some(token) = primary.or(tokens.first()) {
        issue.span = token.span.clone();
    }
    issue.in_source(source)
}

impl Signature {
    /// Formal argument order shared by binding and size/default preparation.
    pub fn parameter_names(&self) -> Vec<&str> {
        self.parameters.iter().map(|p| p.name.as_str()).collect()
    }
}
