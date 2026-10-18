use super::*;

fn workspace() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::at(dir.path());
    (dir, workspace)
}

#[test]
fn adds_and_lists_clients_and_sites() {
    let (_dir, workspace) = workspace();
    add_client(&workspace, " Acme ").unwrap();
    let site = add_site(
        &workspace,
        &AddSiteRequest {
            client: "Acme".into(),
            name: "acme.com".into(),
            format: FormatChoice::Builtin("traefik-clf".into()),
        },
    )
    .unwrap();

    assert_eq!(site.format, "builtin:traefik-clf");
    assert_eq!(list_clients(&workspace).unwrap()[0].name, "Acme");
    assert_eq!(list_sites(&workspace).unwrap(), [site]);
}

#[test]
fn rejects_invalid_input_before_touching_storage() {
    let (_dir, workspace) = workspace();
    assert!(matches!(
        add_client(&workspace, " "),
        Err(CatalogError::InvalidName(NameError::Empty))
    ));
    add_client(&workspace, "Acme").unwrap();
    let error = add_site(
        &workspace,
        &AddSiteRequest {
            client: "Acme".into(),
            name: "acme.com".into(),
            format: FormatChoice::Builtin("lighttpd-xyz".into()),
        },
    )
    .unwrap_err();
    assert!(matches!(error, CatalogError::Format(_)));
    assert_eq!(list_sites(&workspace).unwrap(), Vec::new());
}

#[test]
fn overview_of_an_empty_site() {
    let (_dir, workspace) = workspace();
    add_client(&workspace, "Acme").unwrap();
    add_site(
        &workspace,
        &AddSiteRequest {
            client: "Acme".into(),
            name: "acme.com".into(),
            format: FormatChoice::Auto,
        },
    )
    .unwrap();
    let overview = site_overview(&workspace, "acme.com").unwrap();
    assert_eq!((overview.hits, overview.imports), (0, 0));
    assert_eq!(overview.history, Vec::new());
    let json = serde_json::to_value(&overview).unwrap();
    assert_eq!(json["name"], "acme.com");
}

#[test]
fn reformat_and_remove_sites_then_clients() {
    let (_dir, workspace) = workspace();
    add_client(&workspace, "Acme").unwrap();
    add_site(
        &workspace,
        &AddSiteRequest {
            client: "Acme".into(),
            name: "acme.com".into(),
            format: FormatChoice::Auto,
        },
    )
    .unwrap();
    let updated = set_site_format(
        &workspace,
        "acme.com",
        &FormatChoice::Builtin("combined".into()),
    )
    .unwrap();
    assert_eq!(updated.format, "builtin:combined");
    assert!(set_site_format(&workspace, "acme.com", &FormatChoice::Builtin("x".into())).is_err());

    assert!(matches!(
        remove_client(&workspace, "Acme"),
        Err(CatalogError::Store(StoreError::ClientHasSites(_)))
    ));
    remove_site(&workspace, "acme.com").unwrap();
    assert_eq!(list_sites(&workspace).unwrap(), Vec::new());
    remove_client(&workspace, "Acme").unwrap();
    assert_eq!(list_clients(&workspace).unwrap(), Vec::new());
}
