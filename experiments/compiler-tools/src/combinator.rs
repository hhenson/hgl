use crate::model::{Decl, Expr, Issue, Kind, Parsed, Token, Ty};
use chumsky::prelude::*;

pub fn parse(source: &str, tokens: &[Token]) -> Parsed {
    let input: Vec<_> = tokens.iter().map(|t| t.kind).collect();
    let name = just::<_, &[Kind], extra::Err<Rich<'_, Kind>>>(Kind::Name)
        .map_with(|_, e| source[tokens[e.span().start].span.clone()].to_owned());
    let integer = just(Kind::Int).try_map(|_, span: SimpleSpan| {
        source[tokens[span.start].span.clone()]
            .parse::<i64>()
            .map_err(|_| Rich::custom(span, "integer overflow"))
    });
    let ty = recursive(|ty| {
        let reference = name
            .filter(|n| n == "ref")
            .ignore_then(
                ty.clone()
                    .delimited_by(just(Kind::Less), just(Kind::Greater)),
            )
            .map(|t| Ty::Ref(Box::new(t)));
        let list = name
            .filter(|n| n == "list")
            .ignore_then(
                ty.then_ignore(just(Kind::Comma))
                    .then(integer)
                    .delimited_by(just(Kind::Less), just(Kind::Greater)),
            )
            .try_map(|(t, n), span| {
                usize::try_from(n)
                    .map(|n| Ty::List(Box::new(t), n))
                    .map_err(|_| Rich::custom(span, "list length"))
            });
        choice((
            reference,
            list,
            name.filter(|n| n != "ref" && n != "list")
                .map(|n| if n == "i64" { Ty::I64 } else { Ty::Named(n) }),
        ))
    });
    let field = name.then_ignore(just(Kind::Colon)).then(ty.clone());
    let expression = recursive(|expr| {
        let atom = choice((
            name.map(Expr::Name),
            integer.map(Expr::Int),
            expr.delimited_by(just(Kind::LParen), just(Kind::RParen)),
        ));
        let product = atom
            .clone()
            .foldl(just(Kind::Star).ignore_then(atom).repeated(), |a, b| {
                Expr::Mul(Box::new(a), Box::new(b))
            });
        product
            .clone()
            .foldl(just(Kind::Plus).ignore_then(product).repeated(), |a, b| {
                Expr::Add(Box::new(a), Box::new(b))
            })
    });
    let function = just(Kind::Fn)
        .ignore_then(name)
        .then(
            name.separated_by(just(Kind::Comma))
                .at_least(1)
                .collect::<Vec<_>>()
                .delimited_by(just(Kind::Less), just(Kind::Greater))
                .or_not(),
        )
        .then(
            field
                .clone()
                .separated_by(just(Kind::Comma))
                .collect::<Vec<_>>()
                .delimited_by(just(Kind::LParen), just(Kind::RParen)),
        )
        .then_ignore(just(Kind::Arrow))
        .then(ty)
        .then_ignore(just(Kind::FatArrow))
        .then(expression)
        .map(
            |((((name, generics), params), result), body)| Decl::Function {
                name,
                generics: generics.unwrap_or_default(),
                params,
                result,
                body,
            },
        );
    let structure = just(Kind::Struct)
        .ignore_then(name)
        .then(
            field
                .separated_by(just(Kind::Newline))
                .allow_trailing()
                .collect::<Vec<_>>()
                .padded_by(just(Kind::Newline).repeated())
                .delimited_by(just(Kind::LBrace), just(Kind::RBrace)),
        )
        .map(|(n, f)| Decl::Struct(n, f));
    let boundary = one_of([Kind::Fn, Kind::Struct]);
    let declaration = choice((structure, function))
        .then_ignore(just(Kind::Newline).ignored().or(end()))
        .map(Some)
        .recover_with(via_parser(
            any()
                .ignore_then(any().and_is(boundary.not()).repeated())
                .to(None),
        ));
    let header = just(Kind::Module)
        .ignore_then(name.separated_by(just(Kind::Dot)).at_least(1))
        .then_ignore(just(Kind::Newline))
        .or_not();
    let parser = header.ignore_then(
        declaration
            .padded_by(just(Kind::Newline).repeated())
            .repeated()
            .collect::<Vec<_>>(),
    );
    let (output, errors) = parser.parse(input.as_slice()).into_output_errors();
    Parsed {
        declarations: output.unwrap_or_default().into_iter().flatten().collect(),
        issues: errors
            .into_iter()
            .map(|e| {
                let span = e.span();
                let start = tokens
                    .get(span.start)
                    .map_or(source.len(), |t| t.span.start);
                let end = tokens
                    .get(span.end.saturating_sub(1))
                    .map_or(start, |t| t.span.end);
                Issue {
                    span: start..end,
                    message: format!("{e:?}"),
                }
            })
            .collect(),
    }
}
