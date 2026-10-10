//! Native checking consumes inert descriptors and exact ordinary type bindings.
use hgl_semantics::{library, native_contracts, source_check};

const HEADER: &str = "native_descriptor 1 target \"hgl-rust-1\"\nmodule provider\nprovider \"owning-test\" fingerprint \"contract-v1\"\n";
const SCALAR: &str = "scalar Token canonical \"existing.Token\" capabilities {copy,text,equality,hash,order,serialization}\n";
const HELPER: &str = "value token(text:str)->Token symbol \"token_str\" ownership owned phases {setup,cold,evaluation} effects {allocation}\n";

fn sources(interface: &str, descriptor: &str) -> Vec<(String, String)> {
    vec![
        (
            "provider.hgl".into(),
            format!("module provider\n{interface}\n"),
        ),
        (
            "provider.hgl-native".into(),
            format!("{HEADER}{descriptor}"),
        ),
    ]
}
fn interface() -> &'static str {
    "export native type Token\nnative const fn token(text:str)->Token"
}

#[test]
fn checking_binds_data_without_loading_or_executing_provider_code() -> Result<(), String> {
    let sources = sources(interface(), &format!("{SCALAR}{HELPER}"));
    assert!(source_check::validate_sources(&sources).is_empty());
    let library = library::load(&sources)?;
    let descriptor = &library.native_contracts["provider"];
    assert_eq!(descriptor.scalars["Token"].identity, "existing.Token");
    assert_eq!(descriptor.functions["token"].symbol, "token_str");
    assert!(descriptor.functions["token"].signature.value_function);
    descriptor.verify_image("owning-test", "contract-v1")?;
    assert!(
        descriptor
            .verify_image("other-provider", "contract-v1")
            .is_err()
    );
    assert!(
        descriptor
            .verify_image("owning-test", "wrong-contract")
            .is_err()
    );
    Ok(())
}

#[test]
fn missing_and_incompatible_contracts_fail_before_execution() {
    let cases = [
        (interface(), String::new()),
        (interface(), SCALAR.to_owned()),
        (
            interface(),
            format!("{SCALAR}{}", HELPER.replace("text:str", "text:i64")),
        ),
        (
            interface(),
            format!("{SCALAR}{}", HELPER.replace("->Token", "->str")),
        ),
        (
            interface(),
            format!("{SCALAR}{}", HELPER.replace("->Token", "->Token throws")),
        ),
        (interface(), SCALAR.replace("copy,text", "text")),
        ("native type Token", SCALAR.replace("copy,text", "copy")),
        ("native const fn Token()->i64", SCALAR.to_owned()),
    ];
    for (interface, descriptor) in cases {
        assert!(
            library::load(&sources(interface, &descriptor)).is_err(),
            "{descriptor}"
        );
    }
    assert!(
        library::load(&[(
            "provider.hgl".into(),
            format!("module provider\n{}", interface())
        )])
        .is_err()
    );
}

#[test]
fn canonical_aliases_share_capabilities_and_exact_helper_types() -> Result<(), String> {
    let interface = "native type Token\nnative type Alias\nnative const fn token(text:str)->Alias";
    let alias = SCALAR.replace("scalar Token", "scalar Alias");
    library::load(&sources(interface, &format!("{SCALAR}{alias}{HELPER}")))?;
    assert!(
        library::load(&sources(
            interface,
            &format!(
                "{SCALAR}{}{HELPER}",
                alias.replace(",equality,hash,order", "")
            )
        ))
        .is_err()
    );
    assert!(
        library::load(&sources(
            interface,
            &format!(
                "{SCALAR}{}{HELPER}",
                alias.replace("existing.Token", "existing.Other")
            )
        ))
        .is_err()
    );
    Ok(())
}

#[test]
fn descriptor_format_modes_signals_and_phases_are_checked() {
    for helper in [
        HELPER.replace("text:str", "text:signal"),
        HELPER.replace("text:str", "const text:str"),
        HELPER.replace("text:str", "text:list<signal>"),
        HELPER.replace("ownership owned", "ownership borrowed"),
        HELPER.replace("{setup,cold,evaluation}", "{required_constant}"),
        HELPER.replace("{allocation}", "{blocking}"),
        HELPER.replace("{allocation}", "{mutation}"),
        HELPER.replace("token_str", "crate::arbitrary()"),
    ] {
        assert!(
            library::load(&sources(interface(), &format!("{SCALAR}{helper}"))).is_err(),
            "{helper}"
        );
    }
    for header in [
        HEADER.replace("descriptor 1", "descriptor 2"),
        HEADER.replace("hgl-rust-1", "other-target"),
    ] {
        assert!(
            native_contracts::parse("provider.hgl-native", &format!("{header}{SCALAR}")).is_err()
        );
    }
}

#[test]
fn authored_fixture_has_three_distinct_identities_and_exact_shared_capabilities()
-> Result<(), String> {
    let source =
        include_str!("../../../external/hgraph_std/hgl/hgraph/native/atomic_values_fixture.hgl");
    let descriptor = include_str!("../../../native/fixtures/atomic_values.hgl-native");
    let library = library::load(&[
        ("fixture.hgl".into(), source.into()),
        ("fixture.hgl-native".into(), descriptor.into()),
    ])?;
    let contract = &library.native_contracts["hgraph.std"];
    assert_eq!(contract.scalars["Token"].identity, "hgraph.std::Token");
    assert_eq!(
        contract.scalars["TokenOther"].identity,
        "hgraph.std::TokenOther"
    );
    assert_eq!(
        contract.scalars["TextOnly"].identity,
        "hgraph.std::TextOnly"
    );
    assert_eq!(contract.scalars["Token"].capabilities.len(), 6);
    assert_eq!(contract.scalars["TextOnly"].capabilities.len(), 3);
    assert_eq!(contract.functions.len(), 6);
    Ok(())
}
