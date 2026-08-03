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

const SOURCE_MAPPINGS: [(&str, &str); 25] = [
    ("Intro", "intro-outro"),
    ("Outro", "intro-outro"),
    ("Verse", "bridge"),
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
            PhraseRole::try_new(
                role.id().clone(),
                role.display_name(),
                u16::try_from(index + 1).map_err(|_| PhraseRoleDefaultsError::TooManyRoles)?,
                role.is_archived(),
            )
            .map_err(PhraseRoleDefaultsError::Catalog)
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut mappings = Vec::new();
    for provider in PROVIDERS {
        for (raw_label, role_id) in SOURCE_MAPPINGS {
            mappings.push(SourcePhraseMapping::try_new(
                provider,
                raw_label,
                PhraseRoleId::try_new(role_id)?,
            )?);
        }
        mappings.push(SourcePhraseMapping::try_new(
            provider,
            "Drop",
            PhraseRoleId::try_new("drop")?,
        )?);
        mappings.push(SourcePhraseMapping::try_new(
            provider,
            "*",
            PhraseRoleId::try_new("bridge")?,
        )?);
    }
    Ok(PhraseRoleCatalog::try_new(
        1,
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
}
