//! Source grammar is checked before canonical atomic-scalar normalization.
use hgl_source::{Cursor, lex, value_type};

#[test]
fn ordinary_positions_and_full_type_arguments_remain_distinct() {
    for name in [
        "atomic<i64>",
        "list<atomic<i64>>",
        "tuple<i64,atomic<i64>>",
        "map<i64,atomic<i64>>",
        "set<atomic<i64>>",
        "ref<i64>",
        "rolling<i64,2>",
        "signal",
    ] {
        assert!(!value_type(name), "{name}");
    }
    for name in [
        "i64",
        "tuple<i64,list<f64>>",
        "Box<atomic<i64>>",
        "Box<atomic<list<i64>>>",
        "delta<atomic<list<i64>>>",
        "list<i64,(1+2)>",
    ] {
        assert!(value_type(name), "{name}");
    }
}

#[test]
fn full_type_grammar_validates_each_value_position() {
    for name in [
        "set<atomic<i64>>",
        "map<atomic<i64>,i64>",
        "rolling<atomic<i64>,2>",
        "atomic<list<atomic<i64>>>",
        "Box<atomic<list<atomic<i64>>>>",
        "list<map<atomic<i64>,i64>>",
        "atomic<signal>",
    ] {
        let tokens = lex(name).unwrap();
        assert!(Cursor::new(&tokens).type_name().is_err(), "{name}");
    }
    for name in [
        "map<i64,atomic<list<i64>>>",
        "tuple<atomic<list<i64>>,atomic<i64>>",
        "atomic<Box<atomic<list<i64>>>>",
        "delta<atomic<list<i64>>>",
        "rolling<i64,(3+5),(7+9)>",
        "list<i64,(1+2)>",
    ] {
        let tokens = lex(name).unwrap();
        assert!(Cursor::new(&tokens).type_name().is_ok(), "{name}");
    }
}
