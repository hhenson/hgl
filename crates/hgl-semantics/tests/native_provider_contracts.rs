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
    assert_eq!(
        descriptor.functions["token"].parameters,
        [("text".into(), "str".into())]
    );
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
        HELPER.replace("text:str", "text:str,text:str"),
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
    let descriptor = include_str!("../../../native/stdlib/atomic_values.hgl-native");
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

#[test]
fn checked_metadata_sets_value_mode_only_on_exact_native_declarations() -> Result<(), String> {
    let interface = "native const fn size(value:str)->i64\nfn size(value:i64)->i64 {when {return value}}\nnative const fn legacy(value:signal)->i64";
    let helper = "value size(value:str)->i64 symbol \"length\" ownership owned phases {evaluation} effects {}\n";
    let library = library::load(&sources(interface, helper))?;
    for declaration in library.declarations.iter() {
        let signature = declaration.signature()?;
        assert_eq!(
            signature.value_function,
            declaration.role == library::Role::Native && declaration.name == "size"
        );
    }
    Ok(())
}

#[test]
fn scalar_target_markers_are_explicit_inert_backend_bindings() -> Result<(), String> {
    let marker = SCALAR.trim_end().to_owned() + " marker \"OpaqueToken\"\n";
    let descriptor = format!("{HEADER}{marker}").replace(
        "fingerprint \"contract-v1\"",
        "fingerprint \"contract-v1\" image \"missing.rs\"",
    );
    let parsed = native_contracts::parse("provider.hgl-native", &descriptor)?;
    assert_eq!(parsed.markers["Token"], "OpaqueToken");
    assert_eq!(parsed.scalars["Token"].identity, "existing.Token");
    assert!(
        parsed
            .image
            .as_ref()
            .is_some_and(|p| p.ends_with("missing.rs"))
    );
    assert!(
        native_contracts::parse("provider.hgl-native", &format!("{HEADER}{SCALAR}"))?
            .markers
            .is_empty()
    );
    for invalid in ["", "crate::Token", "Token()", "0Token"] {
        assert!(
            native_contracts::parse(
                "provider.hgl-native",
                &descriptor.replace("OpaqueToken", invalid)
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn canonical_aliases_require_compatible_selected_target_mappings() -> Result<(), String> {
    let scalar = SCALAR.trim_end().to_owned() + " marker \"Token\"\n";
    let mut input = sources("native type Token", &scalar);
    input[1].1 = input[1].1.replace(
        "fingerprint \"contract-v1\"",
        "fingerprint \"contract-v1\" image \"unloaded.rs\"",
    );
    input.push(("alias.hgl".into(), "module alias\nnative type Alias".into()));
    input.push((
        "alias.hgl-native".into(),
        input[1]
            .1
            .replace("module provider", "module alias")
            .replace("scalar Token", "scalar Alias"),
    ));
    library::load(&input)?;
    let mut equivalent = input.clone();
    equivalent[3].0 = "nested/alias.hgl-native".into();
    equivalent[3].1 = equivalent[3]
        .1
        .replace("image \"unloaded.rs\"", "image \"../unloaded.rs\"");
    library::load(&equivalent)?;
    for (from, to) in [
        ("marker \"Token\"", "marker \"Other\""),
        ("provider \"owning-test\"", "provider \"other-test\""),
        ("image \"unloaded.rs\"", "image \"other.rs\""),
        ("fingerprint \"contract-v1\"", "fingerprint \"contract-v2\""),
    ] {
        let mut incompatible = input.clone();
        incompatible[3].1 = incompatible[3].1.replace(from, to);
        let error = library::load(&incompatible)
            .err()
            .ok_or("incompatible target mapping admitted")?;
        assert!(
            error.contains("target mapping") || error.contains("provider image"),
            "{error}"
        );
    }
    Ok(())
}

#[test]
fn shared_provider_metadata_rejects_conflicting_images_even_for_distinct_scalars()
-> Result<(), String> {
    let mut input = sources("native type Token", SCALAR);
    input[1].1 = input[1].1.replace(
        "fingerprint \"contract-v1\"",
        "fingerprint \"contract-v1\" image \"unloaded.rs\"",
    );
    input.push(("other.hgl".into(), "module other\nnative type Other".into()));
    input.push((
        "other.hgl-native".into(),
        input[1]
            .1
            .replace("module provider", "module other")
            .replace("scalar Token", "scalar Other")
            .replace("existing.Token", "existing.Other"),
    ));
    library::load(&input)?;
    for (from, to) in [("unloaded.rs", "other.rs"), ("contract-v1", "contract-v2")] {
        let mut incompatible = input.clone();
        incompatible[3].1 = incompatible[3].1.replace(from, to);
        let error = library::load(&incompatible)
            .err()
            .ok_or("conflicting provider image admitted")?;
        assert!(error.contains("provider image"), "{error}");
    }
    Ok(())
}

#[test]
fn rust_2024_target_keywords_fail_in_every_descriptor_role() {
    for identifier in [
        "gen", "async", "await", "crate", "self", "Self", "super", "_",
    ] {
        for descriptor in [
            format!("{SCALAR}{}", HELPER.replace("token_str", identifier)),
            format!(
                "{SCALAR}{}",
                HELPER.replace("ownership", &format!("bound {identifier:?} ownership"))
            ),
            format!("{} marker {identifier:?}\n{HELPER}", SCALAR.trim_end()),
        ] {
            let error =
                native_contracts::parse("provider.hgl-native", &format!("{HEADER}{descriptor}"))
                    .unwrap_err();
            assert!(
                error.to_string().contains("Rust 2024 target identifier"),
                "{identifier}: {error}"
            );
        }
    }
}

#[test]
fn hgl_names_and_canonical_identities_do_not_use_rust_keyword_rules() -> Result<(), String> {
    let scalar = SCALAR
        .replace("scalar Token", "scalar gen")
        .replace("existing.Token", "existing.gen");
    let helper = HELPER
        .replace("value token", "value gen")
        .replace("->Token", "->gen");
    let library = library::load(&sources(
        "native type gen\nnative const fn gen(text:str)->gen",
        &format!("{scalar}{helper}"),
    ))?;
    assert_eq!(
        library.native_contracts["provider"].scalars["gen"].identity,
        "existing.gen"
    );
    assert_eq!(
        library.native_contracts["provider"].functions["gen"].symbol,
        "token_str"
    );
    for identifier in ["generate", "gen_value", "_gen", "union", "crate_value"] {
        native_contracts::parse(
            "provider.hgl-native",
            &format!(
                "{HEADER}{SCALAR}{}",
                HELPER.replace("token_str", identifier)
            ),
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
