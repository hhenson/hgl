use hgl_compiler_tools_spike::{check, emit, hand, lex};
use std::{hint::black_box, time::Instant};
fn measure(name: &str, count: u32, mut f: impl FnMut()) {
    let start = Instant::now();
    for _ in 0..count {
        f();
    }
    println!("{name},{count},{}", start.elapsed().as_nanos());
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let source = include_str!("../corpus/valid.hgl");
    let raw = lex::handwritten(source);
    let tokens = lex::significant(source, &raw);
    let program = check::check(&hand::parse(source, &tokens))?;
    if args.get(1).map(String::as_str) == Some("emit") {
        let out = std::path::Path::new(args.get(2).ok_or("output directory required")?);
        std::fs::create_dir_all(out)?;
        std::fs::write(out.join("hand-text.rs"), emit::text(&program))?;
        #[cfg(all(
            feature = "quote",
            feature = "proc-macro2",
            feature = "syn",
            feature = "prettyplease"
        ))]
        std::fs::write(out.join("hand-tokens.rs"), emit::tokens(&program))?;
        #[cfg(all(
            feature = "logos",
            feature = "chumsky",
            feature = "quote",
            feature = "proc-macro2",
            feature = "syn",
            feature = "prettyplease"
        ))]
        {
            let raw = lex::generated(source);
            let tokens = lex::significant(source, &raw);
            let candidate = check::check(&hgl_compiler_tools_spike::combinator::parse(
                source, &tokens,
            ))?;
            if candidate != program {
                return Err("candidate checked programs disagree".into());
            }
            std::fs::write(out.join("candidate-text.rs"), emit::text(&candidate))?;
            std::fs::write(out.join("candidate-tokens.rs"), emit::tokens(&candidate))?;
        }
        return Ok(());
    }
    let count = 2000;
    println!("case,iterations,nanoseconds");
    measure("hand_lex", count, || {
        black_box(lex::handwritten(black_box(source)));
    });
    #[cfg(feature = "logos")]
    measure("logos_lex", count, || {
        black_box(lex::generated(black_box(source)));
    });
    measure("hand_parse", count, || {
        black_box(hand::parse(black_box(source), black_box(&tokens)));
    });
    #[cfg(feature = "chumsky")]
    measure("chumsky_parse", count, || {
        black_box(hgl_compiler_tools_spike::combinator::parse(
            black_box(source),
            black_box(&tokens),
        ));
    });
    measure("hand_pipeline", count, || {
        let raw = lex::handwritten(black_box(source));
        black_box(hand::parse(source, &lex::significant(source, &raw)));
    });
    #[cfg(all(feature = "logos", feature = "chumsky"))]
    measure("candidate_pipeline", count, || {
        let raw = lex::generated(black_box(source));
        black_box(hgl_compiler_tools_spike::combinator::parse(
            source,
            &lex::significant(source, &raw),
        ));
    });
    let broken = include_str!("../corpus/recovery.hgl");
    let errors = lex::significant(broken, &lex::handwritten(broken));
    measure("hand_recovery", count, || {
        black_box(hand::parse(black_box(broken), black_box(&errors)));
    });
    #[cfg(feature = "chumsky")]
    measure("chumsky_recovery", count, || {
        black_box(hgl_compiler_tools_spike::combinator::parse(
            black_box(broken),
            black_box(&errors),
        ));
    });
    measure("text_emit", count, || {
        black_box(emit::text(black_box(&program)));
    });
    #[cfg(all(
        feature = "quote",
        feature = "proc-macro2",
        feature = "syn",
        feature = "prettyplease"
    ))]
    measure("token_emit_formatted", count, || {
        black_box(emit::tokens(black_box(&program)));
    });
    #[cfg(all(feature = "syn", feature = "prettyplease"))]
    measure("text_emit_formatted", count, || {
        let text = emit::text(black_box(&program));
        black_box(prettyplease::unparse(&syn::parse_file(&text).unwrap()));
    });
    let large: String = (0..100)
        .map(|n| format!("fn f{n}(x: i64, y: i64) -> i64 => (x + y) * 2\n"))
        .collect();
    let large_tokens = lex::significant(&large, &lex::handwritten(&large));
    measure("hand_parse_100_functions", 100, || {
        black_box(hand::parse(black_box(&large), black_box(&large_tokens)));
    });
    #[cfg(feature = "chumsky")]
    measure("chumsky_parse_100_functions", 100, || {
        black_box(hgl_compiler_tools_spike::combinator::parse(
            black_box(&large),
            black_box(&large_tokens),
        ));
    });
    #[cfg(feature = "rowan")]
    measure("rowan_storage_roundtrip", count, || {
        black_box(hgl_compiler_tools_spike::tooling::lossless(
            black_box(source),
            black_box(&raw),
        ));
    });
    Ok(())
}
