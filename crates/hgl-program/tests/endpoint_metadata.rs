//! Endpoint operation admission must agree with the available typed emission paths.
use hgl_program::{compile, emit_rust};

const STRUCTURAL: [(&str, &str); 6] = [
    ("list<i64,2>", "delta<list<i64,2>>(items:[0:1])"),
    ("map<i64,i64>", "delta<map<i64,i64>>(upsert:[1:1])"),
    ("tuple<i64,str>", "delta<tuple<i64,str>>(items:[0:1])"),
    ("Quote", "delta<Quote>(value:1)"),
    ("set<i64>", "delta<set<i64>>(added:[1])"),
    ("set<bool>", "delta<set<bool>>(added:[false])"),
];

fn source(shape: &str, delta: &str, operation: &str) -> Vec<(String, String)> {
    vec![(
        "metadata.hgl".into(),
        format!(
            "module metadata\nstruct Quote {{value:i64}}\nfn source()->{shape} {{yield 0us:{delta}}}\nfn observe(value:{shape})->{shape} {{inject out\nwhen {{{operation}\nreturn delta_value(value)}}}}\nfn main()->{shape} => observe(source())"
        ),
    )]
}

#[test]
fn structural_activity_is_rejected_before_emission() {
    for (shape, delta) in STRUCTURAL {
        for operation in ["activate(value)", "passivate(value)"] {
            let error = compile(&source(shape, delta, operation), "main").unwrap_err();
            assert!(
                error.contains("activity requires a scalar input"),
                "{shape} {operation}: {error}"
            );
        }
    }
}

#[test]
fn structural_output_metadata_is_rejected_before_emission() {
    for (shape, delta) in STRUCTURAL {
        for operation in [
            "valid(out)",
            "modified(out)",
            "last_modified(out)",
            "valid(value,out)",
        ] {
            let error = compile(&source(shape, delta, operation), "main").unwrap_err();
            assert!(
                error.contains("querying structural out is not yet supported"),
                "{shape} {operation}: {error}"
            );
        }
    }
}

#[test]
fn structural_input_metadata_and_scalar_operations_still_emit() {
    for (shape, delta) in STRUCTURAL {
        let program=compile(&source(shape, delta, "let ready=valid(value)\nlet changed=modified(value)\nlet time=last_modified(value)"),"main").unwrap();
        let code = emit_rust(&program);
        assert!(code.contains("input_valid(self.input0.id())"), "{shape}");
        assert!(code.contains("last_modified(self.input0.id())"), "{shape}");
    }
    let program=compile(&source("i64","1","activate(value)\npassivate(value)\nlet ready=valid(out)\nlet changed=modified(out)\nlet time=last_modified(out)"),"main").unwrap();
    let code = emit_rust(&program);
    assert!(code.contains("set_active(self.input0,true)"));
    assert!(code.contains("set_active(self.input0,false)"));
    assert!(code.contains("output_ref(self._output).is_some()"));
}
