use crate::model::{Decl, Expr, Ty};

fn expression(e: &Expr) -> String {
    match e {
        Expr::Name(n) => format!("arg_{n}"),
        Expr::Int(n) => n.to_string(),
        Expr::Add(a, b) => format!("({} + {})", expression(a), expression(b)),
        Expr::Mul(a, b) => format!("({} * {})", expression(a), expression(b)),
    }
}
fn body_expression(e: &Expr) -> String {
    let text = expression(e);
    match e {
        Expr::Add(..) | Expr::Mul(..) => text[1..text.len() - 1].to_owned(),
        _ => text,
    }
}
fn value_type(t: &Ty) -> &'static str {
    if *t == Ty::I64 {
        "i64"
    } else {
        "hgl_store::Reference"
    }
}
fn shape(t: &Ty) -> String {
    match t {
        Ty::I64 => "hgl_types::TsType::Ts(hgl_types::ScalarType::I64)".into(),
        Ty::Ref(t) => format!("hgl_types::TsType::Reference(Box::new({}))", shape(t)),
        Ty::List(t, n) => format!("hgl_types::TsType::List(Box::new({}), {n})", shape(t)),
        Ty::Bundle(fields) => format!(
            "hgl_types::TsType::Bundle(vec![{}])",
            fields
                .iter()
                .map(|(n, t)| format!("({n:?}.into(), {})", shape(t)))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Ty::Named(_) => unreachable!("checker must resolve named shapes"),
    }
}
pub fn text(program: &[Decl]) -> String {
    let mut result = String::new();
    for d in program {
        let Decl::Function {
            name,
            params,
            result: ty,
            body,
            ..
        } = d
        else {
            continue;
        };
        let params = params
            .iter()
            .map(|(n, t)| format!("arg_{n}: {}", value_type(t)))
            .collect::<Vec<_>>()
            .join(",");
        result.push_str(&format!(
            "pub fn hgl_{name}({params}) -> {} {{ {} }}\n",
            value_type(ty),
            body_expression(body)
        ));
        result.push_str(&format!(
            "pub fn shape_{name}() -> hgl_types::TsType {{ {} }}\n",
            shape(ty)
        ));
    }
    result
}
#[cfg(all(
    feature = "quote",
    feature = "proc-macro2",
    feature = "syn",
    feature = "prettyplease"
))]
pub fn tokens(program: &[Decl]) -> String {
    use quote::{format_ident, quote};
    fn expr(e: &Expr) -> proc_macro2::TokenStream {
        match e {
            Expr::Name(n) => {
                let n = format_ident!("arg_{n}");
                quote!(#n)
            }
            Expr::Int(n) => quote!(#n),
            Expr::Add(a, b) => {
                let (a, b) = (expr(a), expr(b));
                quote!((#a + #b))
            }
            Expr::Mul(a, b) => {
                let (a, b) = (expr(a), expr(b));
                quote!((#a * #b))
            }
        }
    }
    fn value(t: &Ty) -> proc_macro2::TokenStream {
        if *t == Ty::I64 {
            quote!(i64)
        } else {
            quote!(hgl_store::Reference)
        }
    }
    fn ty(t: &Ty) -> proc_macro2::TokenStream {
        match t {
            Ty::I64 => quote!(hgl_types::TsType::Ts(hgl_types::ScalarType::I64)),
            Ty::Ref(t) => {
                let t = ty(t);
                quote!(hgl_types::TsType::Reference(Box::new(#t)))
            }
            Ty::List(t, n) => {
                let t = ty(t);
                quote!(hgl_types::TsType::List(Box::new(#t), #n))
            }
            Ty::Bundle(fields) => {
                let fields = fields.iter().map(|(n, t)| {
                    let t = ty(t);
                    quote!((#n.into(), #t))
                });
                quote!(hgl_types::TsType::Bundle(vec![#(#fields),*]))
            }
            Ty::Named(_) => unreachable!("checker must resolve named shapes"),
        }
    }
    let functions = program.iter().filter_map(|d| {
        let Decl::Function {
            name,
            params,
            result,
            body,
            ..
        } = d
        else {
            return None;
        };
        let function = format_ident!("hgl_{name}");
        let signature = format_ident!("shape_{name}");
        let args = params.iter().map(|(n, t)| {
            let n = format_ident!("arg_{n}");
            let t = value(t);
            quote!(#n: #t)
        });
        let result_type = value(result);
        let shape = ty(result);
        let body = match body {
            Expr::Add(a, b) => {
                let (a, b) = (expr(a), expr(b));
                quote!(#a + #b)
            }
            Expr::Mul(a, b) => {
                let (a, b) = (expr(a), expr(b));
                quote!(#a * #b)
            }
            e => expr(e),
        };
        Some(quote! {
            pub fn #function(#(#args),*) -> #result_type { #body }
            pub fn #signature() -> hgl_types::TsType { #shape }
        })
    });
    let output = quote!(#(#functions)*);
    prettyplease::unparse(&syn::parse2(output).expect("emitter produced invalid Rust"))
}
