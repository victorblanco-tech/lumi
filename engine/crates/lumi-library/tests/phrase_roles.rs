use lumi_library::{
    PHRASE_ROLE_DEFAULTS_VERSION, PhraseRole, PhraseRoleCatalog, PhraseRoleCatalogError,
    PhraseRoleId, PhraseRoleMove, SourcePhraseMapping,
};

fn role(id: &str, name: &str, order: u16) -> Result<PhraseRole, Box<dyn std::error::Error>> {
    Ok(PhraseRole::try_new(
        PhraseRoleId::try_new(id)?,
        name,
        order,
        false,
    )?)
}

fn catalog() -> Result<PhraseRoleCatalog, Box<dyn std::error::Error>> {
    PhraseRoleCatalog::try_new(
        1,
        PHRASE_ROLE_DEFAULTS_VERSION,
        vec![
            role("intro-outro", "Intro / Outro", 1)?,
            role("synth", "Synth", 2)?,
        ],
        vec![SourcePhraseMapping::try_new(
            "rekordbox7",
            "Intro",
            PhraseRoleId::try_new("intro-outro")?,
        )?],
    )
    .map_err(Into::into)
}

#[test]
fn rename_and_reorder_preserve_stable_ids() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = catalog()?;
    let renamed = catalog.rename_role(&PhraseRoleId::try_new("synth")?, "Lead Synth")?;
    let moved = renamed.move_role(&PhraseRoleId::try_new("synth")?, PhraseRoleMove::Earlier)?;

    assert_eq!(moved.revision(), 3);
    assert_eq!(moved.roles()[0].id().as_str(), "synth");
    assert_eq!(moved.roles()[0].display_name(), "Lead Synth");
    assert_eq!(moved.roles()[1].id().as_str(), "intro-outro");
    assert_eq!(
        moved
            .resolve("rekordbox7", "  INTRO ")
            .map(PhraseRoleId::as_str),
        Some("intro-outro")
    );
    Ok(())
}

#[test]
fn custom_ids_are_stable_and_never_derived_from_a_rename() -> Result<(), Box<dyn std::error::Error>>
{
    let added = catalog()?.add_role("Vocal Lift")?;
    let id = added.roles()[2].id().clone();
    let renamed = added.rename_role(&id, "Vocal Peak")?;

    assert_eq!(id.as_str(), "custom-1");
    assert_eq!(renamed.roles()[2].id(), &id);
    assert_eq!(renamed.roles()[2].display_name(), "Vocal Peak");
    Ok(())
}

#[test]
fn archive_is_reversible_and_cannot_remove_the_last_active_role()
-> Result<(), Box<dyn std::error::Error>> {
    let first = catalog()?.set_archived(&PhraseRoleId::try_new("intro-outro")?, true)?;
    assert!(first.roles()[0].is_archived());
    let error = first
        .set_archived(&PhraseRoleId::try_new("synth")?, true)
        .err()
        .ok_or("archiving the last role unexpectedly succeeded")?;
    assert_eq!(error, PhraseRoleCatalogError::NoActiveRoles);

    let restored = first.set_archived(&PhraseRoleId::try_new("intro-outro")?, false)?;
    assert!(!restored.roles()[0].is_archived());
    Ok(())
}

#[test]
fn source_mapping_is_provider_scoped_and_rejects_duplicates()
-> Result<(), Box<dyn std::error::Error>> {
    let catalog = catalog()?.upsert_mapping(SourcePhraseMapping::try_new(
        "demo",
        "Intro",
        PhraseRoleId::try_new("synth")?,
    )?)?;
    assert_eq!(
        catalog.resolve("demo", "intro").map(PhraseRoleId::as_str),
        Some("synth")
    );
    assert_eq!(
        catalog
            .resolve("rekordbox7", "intro")
            .map(PhraseRoleId::as_str),
        Some("intro-outro")
    );

    let duplicate = PhraseRoleCatalog::try_new(
        1,
        PHRASE_ROLE_DEFAULTS_VERSION,
        vec![role("intro-outro", "Intro / Outro", 1)?],
        vec![
            SourcePhraseMapping::try_new(
                "rekordbox7",
                "Intro",
                PhraseRoleId::try_new("intro-outro")?,
            )?,
            SourcePhraseMapping::try_new(
                "REKORDBOX7",
                " intro ",
                PhraseRoleId::try_new("intro-outro")?,
            )?,
        ],
    )
    .err()
    .ok_or("duplicate mapping unexpectedly succeeded")?;
    assert_eq!(duplicate, PhraseRoleCatalogError::DuplicateMapping);
    Ok(())
}

#[test]
fn intensity_roles_do_not_require_sequential_completeness() -> Result<(), Box<dyn std::error::Error>>
{
    let catalog = PhraseRoleCatalog::try_new(
        1,
        PHRASE_ROLE_DEFAULTS_VERSION,
        vec![
            role("breakdown-3", "Breakdown 3", 1)?,
            role("buildup-2", "Buildup 2", 2)?,
        ],
        vec![],
    )?;

    assert_eq!(catalog.roles().len(), 2);
    assert_eq!(catalog.roles()[0].id().as_str(), "breakdown-3");
    assert_eq!(catalog.roles()[1].id().as_str(), "buildup-2");
    Ok(())
}
