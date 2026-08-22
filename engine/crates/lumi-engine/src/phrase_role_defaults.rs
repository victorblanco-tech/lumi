use lumi_library::{
    PHRASE_ROLE_DEFAULTS_VERSION, PhraseRole, PhraseRoleCatalog, PhraseRoleCatalogError,
    PhraseRoleId, SourcePhraseMapping, TextIdentifierError,
};
use thiserror::Error;

const ROLE_DEFINITIONS: [(&str, &str); 11] = [
    ("intro-outro", "Intro / Outro"),
    ("bridge", "Bridge"),
    ("breakdown-1", "Breakdown 1"),
    ("breakdown-2", "Breakdown 2"),
    ("breakdown-3", "Breakdown 3"),
    ("synth", "Synth"),
    ("pre-drop", "Pre-drop"),
    ("buildup-1", "Buildup 1"),
    ("buildup-2", "Buildup 2"),
    ("buildup-3", "Buildup 3"),
    ("drop", "Drop"),
];

const SOURCE_MAPPINGS: [(&str, &str); 38] = [
    ("Intro", "intro-outro"),
    ("Intro 1", "intro-outro"),
    ("Intro 2", "intro-outro"),
    ("Outro", "intro-outro"),
    ("Outro 1", "intro-outro"),
    ("Outro 2", "intro-outro"),
    ("Verse", "bridge"),
    ("Verse 1", "bridge"),
    ("Verse 2", "bridge"),
    ("Verse 3", "bridge"),
    ("Verse 4", "bridge"),
    ("Verse 5", "bridge"),
    ("Verse 6", "bridge"),
    ("Bridge", "bridge"),
    ("Breakdown", "breakdown-1"),
    ("Breakdown 1", "breakdown-1"),
    ("Down", "breakdown-1"),
    ("Down 1", "breakdown-1"),
    ("Breakdown 2", "breakdown-2"),
    ("Down 2", "breakdown-2"),
    ("Breakdown 3", "breakdown-3"),
    ("Down 3", "breakdown-3"),
    ("Synth", "synth"),
    ("Pre-drop", "pre-drop"),
    ("Predrop", "pre-drop"),
    ("Up", "buildup-1"),
    ("Up 1", "buildup-1"),
    ("Build", "buildup-1"),
    ("Buildup", "buildup-1"),
    ("Buildup 1", "buildup-1"),
    ("Up 2", "buildup-2"),
    ("Buildup 2", "buildup-2"),
    ("Up 3", "buildup-3"),
    ("Buildup 3", "buildup-3"),
    ("Chorus", "drop"),
    ("Chorus 1", "drop"),
    ("Chorus 2", "drop"),
    ("Drop", "drop"),
];

const PROVIDERS: [&str; 2] = ["demo", "rekordbox7"];

pub fn seeded_phrase_role_catalog(
    existing: &PhraseRoleCatalog,
) -> Result<PhraseRoleCatalog, PhraseRoleDefaultsError> {
    if existing.defaults_version() >= PHRASE_ROLE_DEFAULTS_VERSION {
        return Ok(existing.clone());
    }
    let mut roles = existing.roles().to_vec();
    for (id, name) in ROLE_DEFINITIONS {
        let id = PhraseRoleId::try_new(id)?;
        if !roles.iter().any(|role| role.id() == &id) {
            roles.push(PhraseRole::try_new(
                id,
                name,
                u16::try_from(roles.len() + 1)
                    .map_err(|_| PhraseRoleDefaultsError::TooManyRoles)?,
                false,
            )?);
        }
    }
    roles = roles
        .into_iter()
        .enumerate()
        .map(|(index, role)| {
            PhraseRole::try_new_with_color_rgb(
                role.id().clone(),
                role.display_name(),
                u16::try_from(index + 1).map_err(|_| PhraseRoleDefaultsError::TooManyRoles)?,
                role.is_archived(),
                role.color_rgb(),
            )
            .map_err(PhraseRoleDefaultsError::Catalog)
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Defaults are additive. Existing user mappings always win during an
    // upgrade, including a deliberately customized wildcard.
    let mut mappings = existing.mappings().to_vec();
    for provider in PROVIDERS {
        for (raw_label, role_id) in SOURCE_MAPPINGS {
            let mapping =
                SourcePhraseMapping::try_new(provider, raw_label, PhraseRoleId::try_new(role_id)?)?;
            if !mappings.iter().any(|existing| {
                existing.provider_kind().eq_ignore_ascii_case(provider)
                    && existing.normalized_label() == mapping.normalized_label()
            }) {
                mappings.push(mapping);
            }
        }
        let wildcard =
            SourcePhraseMapping::try_new(provider, "*", PhraseRoleId::try_new("bridge")?)?;
        if !mappings.iter().any(|existing| {
            existing.provider_kind().eq_ignore_ascii_case(provider)
                && existing.normalized_label() == "*"
        }) {
            mappings.push(wildcard);
        }
    }
    mappings.sort_by(|left, right| {
        left.provider_kind()
            .cmp(right.provider_kind())
            .then_with(|| left.normalized_label().cmp(&right.normalized_label()))
    });
    let revision = if existing.revision() == 0 {
        1
    } else {
        existing
            .revision()
            .checked_add(1)
            .ok_or(PhraseRoleDefaultsError::RevisionOverflow)?
    };
    Ok(PhraseRoleCatalog::try_new(
        revision,
        PHRASE_ROLE_DEFAULTS_VERSION,
        roles,
        mappings,
    )?)
}

#[must_use]
pub fn provider_display_name(provider_kind: &str) -> &str {
    match provider_kind {
        "demo" => "Demo Library",
        "rekordbox7" => "Rekordbox 7",
        value => value,
    }
}

#[derive(Debug, Error)]
pub enum PhraseRoleDefaultsError {
    #[error("invalid phrase-role identifier: {0}")]
    Identifier(#[from] TextIdentifierError),
    #[error("invalid phrase-role defaults: {0}")]
    Catalog(#[from] PhraseRoleCatalogError),
    #[error("phrase-role defaults contain too many roles")]
    TooManyRoles,
    #[error("phrase-role catalog revision overflowed")]
    RevisionOverflow,
}

#[cfg(test)]
mod tests {
    use lumi_library::{PhraseRoleCatalog, PhraseRoleId, SourcePhraseMapping};

    use super::seeded_phrase_role_catalog;

    fn empty_catalog() -> Result<PhraseRoleCatalog, Box<dyn std::error::Error>> {
        Ok(PhraseRoleCatalog::try_new(0, 0, Vec::new(), Vec::new())?)
    }

    #[test]
    fn rekordbox_phrase_variants_map_without_falling_back_to_bridge()
    -> Result<(), Box<dyn std::error::Error>> {
        let catalog = seeded_phrase_role_catalog(&empty_catalog()?)?;
        for label in ["Intro 1", "Intro 2", "Outro 1", "Outro 2"] {
            assert_eq!(
                catalog
                    .resolve("rekordbox7", label)
                    .map(PhraseRoleId::as_str),
                Some("intro-outro")
            );
        }
        for label in [
            "Verse 1", "Verse 2", "Verse 3", "Verse 4", "Verse 5", "Verse 6",
        ] {
            assert_eq!(
                catalog
                    .resolve("rekordbox7", label)
                    .map(PhraseRoleId::as_str),
                Some("bridge")
            );
        }
        for label in ["Chorus", "Chorus 1", "Chorus 2"] {
            assert_eq!(
                catalog
                    .resolve("rekordbox7", label)
                    .map(PhraseRoleId::as_str),
                Some("drop")
            );
        }
        Ok(())
    }

    #[test]
    fn defaults_upgrade_preserves_customized_existing_mapping()
    -> Result<(), Box<dyn std::error::Error>> {
        let seeded = seeded_phrase_role_catalog(&empty_catalog()?)?;
        let customized = seeded.upsert_mapping(SourcePhraseMapping::try_new(
            "rekordbox7",
            "Chorus 2",
            PhraseRoleId::try_new("synth")?,
        )?)?;
        let version_one = PhraseRoleCatalog::try_new(
            customized.revision(),
            1,
            customized.roles().to_vec(),
            customized.mappings().to_vec(),
        )?;
        let upgraded = seeded_phrase_role_catalog(&version_one)?;
        assert_eq!(
            upgraded
                .resolve("rekordbox7", "Chorus 2")
                .map(PhraseRoleId::as_str),
            Some("synth")
        );
        Ok(())
    }
}
