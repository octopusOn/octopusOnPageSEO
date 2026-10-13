use super::*;

fn catalog() -> (tempfile::TempDir, Catalog) {
    let dir = tempfile::tempdir().unwrap();
    let catalog = Catalog::open(&DataDir::new(dir.path())).unwrap();
    (dir, catalog)
}

#[test]
fn clients_and_sites_round_trip() {
    let (_dir, catalog) = catalog();
    let acme = catalog.add_client("Acme").unwrap();
    catalog.add_client("Beta").unwrap();
    let site = catalog
        .add_site(NewSite {
            client_name: "Acme",
            name: "acme.com",
            format_spec: "builtin:traefik-clf",
        })
        .unwrap();

    assert_eq!(site.client_id, acme.id);
    assert_eq!(catalog.site_by_name("acme.com").unwrap(), site);
    let names: Vec<_> = catalog
        .clients()
        .unwrap()
        .into_iter()
        .map(|c| c.name)
        .collect();
    assert_eq!(names, ["Acme", "Beta"]);
    assert_eq!(catalog.sites().unwrap(), [site]);
}

#[test]
fn duplicates_and_missing_references_are_rejected() {
    let (_dir, catalog) = catalog();
    catalog.add_client("Acme").unwrap();
    assert!(matches!(
        catalog.add_client("Acme"),
        Err(StoreError::DuplicateClient(_))
    ));
    let new_site = NewSite {
        client_name: "Nobody",
        name: "x.com",
        format_spec: "auto",
    };
    assert!(matches!(
        catalog.add_site(new_site),
        Err(StoreError::ClientNotFound(_))
    ));
    let new_site = NewSite {
        client_name: "Acme",
        ..new_site
    };
    catalog.add_site(new_site).unwrap();
    assert!(matches!(
        catalog.add_site(new_site),
        Err(StoreError::DuplicateSite(_))
    ));
    assert!(matches!(
        catalog.site_by_name("y.com"),
        Err(StoreError::SiteNotFound(_))
    ));
}

#[test]
fn data_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let data = DataDir::new(dir.path());
    Catalog::open(&data).unwrap().add_client("Acme").unwrap();
    assert_eq!(Catalog::open(&data).unwrap().clients().unwrap().len(), 1);
}

#[test]
fn sites_can_be_reformatted_and_removed_before_their_client() {
    let (_dir, catalog) = catalog();
    catalog.add_client("Acme").unwrap();
    let site = catalog
        .add_site(NewSite {
            client_name: "Acme",
            name: "acme.com",
            format_spec: "auto",
        })
        .unwrap();
    catalog
        .set_site_format(site.id, "builtin:combined")
        .unwrap();
    assert_eq!(
        catalog.site_by_name("acme.com").unwrap().format_spec,
        "builtin:combined"
    );
    assert!(matches!(
        catalog.remove_client("Acme"),
        Err(StoreError::ClientHasSites(_))
    ));
    catalog.remove_site(site.id).unwrap();
    assert!(matches!(
        catalog.remove_site(site.id),
        Err(StoreError::SiteNotFound(_))
    ));
    catalog.remove_client("Acme").unwrap();
    assert_eq!(catalog.clients().unwrap(), Vec::new());
}

#[test]
fn concurrent_threads_take_turns() {
    let dir = tempfile::tempdir().unwrap();
    let data = DataDir::new(dir.path());
    Catalog::open(&data).unwrap();
    std::thread::scope(|scope| {
        for n in 0..8 {
            let data = &data;
            scope.spawn(move || {
                let catalog = Catalog::open(data).unwrap();
                catalog.add_client(&format!("client-{n}")).unwrap();
            });
        }
    });
    assert_eq!(Catalog::open(&data).unwrap().clients().unwrap().len(), 8);
}
