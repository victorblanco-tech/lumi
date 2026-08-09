use lumi_domain::{ThemeId, TrackId};
use lumi_library::{
    LumiPhraseTimeline, PhraseAbsorption, PhraseInstance, PhraseLoopStrategy, PhraseRoleId,
    SourceRevision, ThemeSpecificVariant, TimelineEditCommand, TimelineEditError, TimelineRevision,
    TimelineRevisionOrigin, TimelineRevisionReason, VariantId,
};

fn role(value: &str) -> Result<PhraseRoleId, lumi_library::TextIdentifierError> {
    PhraseRoleId::try_new(value)
}

fn timeline() -> Result<LumiPhraseTimeline, Box<dyn std::error::Error>> {
    Ok(LumiPhraseTimeline::try_new_with_history(
        TrackId::new(7),
        TimelineRevision::initial(),
        SourceRevision::try_new("analysis-v1")?,
        16,
        TimelineRevisionOrigin::SourceImport,
        TimelineRevisionReason::InitialSourceMapping,
        None,
        None,
        vec![
            PhraseInstance::new(0, 0, 4, role("intro")?),
            PhraseInstance::new(1, 4, 8, role("breakdown-1")?).with_loop_strategy(
                PhraseLoopStrategy::FixedVariant(VariantId::try_new("variant-2")?),
            ),
            PhraseInstance::new(2, 8, 12, role("buildup-1")?),
            PhraseInstance::new(3, 12, 16, role("drop")?),
        ],
    )?)
}

fn assert_canonical(timeline: &LumiPhraseTimeline) {
    assert!(!timeline.phrases().is_empty());
    assert_eq!(timeline.phrases()[0].start_beat(), 0);
    assert_eq!(
        timeline.phrases().last().map(PhraseInstance::end_beat),
        Some(16)
    );
    for (index, phrase) in timeline.phrases().iter().enumerate() {
        assert_eq!(usize::from(phrase.index()), index);
        assert!(phrase.start_beat() < phrase.end_beat());
        if index > 0 {
            assert_eq!(
                timeline.phrases()[index - 1].end_beat(),
                phrase.start_beat()
            );
        }
    }
}

#[test]
fn every_edit_preserves_contiguous_beat_coverage_and_history()
-> Result<(), Box<dyn std::error::Error>> {
    let commands = [
        TimelineEditCommand::Create {
            start_beat: 2,
            end_beat: 6,
            role_id: role("synth")?,
        },
        TimelineEditCommand::Split {
            phrase_index: 1,
            at_beat: 5,
        },
        TimelineEditCommand::MergePrevious { phrase_index: 2 },
        TimelineEditCommand::MergeNext { phrase_index: 1 },
        TimelineEditCommand::MoveBoundary {
            boundary_after_phrase_index: 1,
            to_beat: 10,
        },
        TimelineEditCommand::Delete {
            phrase_index: 1,
            absorb_into: PhraseAbsorption::Previous,
        },
        TimelineEditCommand::Delete {
            phrase_index: 2,
            absorb_into: PhraseAbsorption::Next,
        },
        TimelineEditCommand::ChangeRole {
            phrase_index: 2,
            role_id: role("synth")?,
        },
        TimelineEditCommand::SetLoopStrategy {
            phrase_index: 0,
            strategy: PhraseLoopStrategy::FixedVariant(VariantId::try_new("variant-1")?),
        },
    ];

    for command in commands {
        let original = timeline()?;
        let edited = original.edit(command)?;
        assert_canonical(&edited);
        assert_eq!(edited.revision(), TimelineRevision::try_new(2)?);
        assert_eq!(edited.parent_revision(), Some(original.revision()));
        assert_eq!(edited.origin(), TimelineRevisionOrigin::UserEdit);
    }
    Ok(())
}

#[test]
fn strategy_edits_are_revisioned_and_role_changes_reset_to_auto()
-> Result<(), Box<dyn std::error::Error>> {
    let fixed = timeline()?.edit(TimelineEditCommand::SetLoopStrategy {
        phrase_index: 0,
        strategy: PhraseLoopStrategy::FixedVariant(VariantId::try_new("variant-1")?),
    })?;
    assert_eq!(fixed.reason(), TimelineRevisionReason::ChangeLoopStrategy);
    assert!(matches!(
        fixed.phrases()[0].loop_strategy(),
        PhraseLoopStrategy::FixedVariant(variant) if variant.as_str() == "variant-1"
    ));

    let automatic = fixed.edit(TimelineEditCommand::SetLoopStrategy {
        phrase_index: 0,
        strategy: PhraseLoopStrategy::Auto,
    })?;
    assert_eq!(
        automatic.phrases()[0].loop_strategy(),
        &PhraseLoopStrategy::Auto
    );

    let changed_role = timeline()?.edit(TimelineEditCommand::ChangeRole {
        phrase_index: 1,
        role_id: role("synth")?,
    })?;
    assert_eq!(
        changed_role.phrases()[1].loop_strategy(),
        &PhraseLoopStrategy::Auto
    );
    Ok(())
}

#[test]
fn split_keeps_exact_choice_left_and_resets_new_right_to_auto()
-> Result<(), Box<dyn std::error::Error>> {
    let edited = timeline()?.edit(TimelineEditCommand::Split {
        phrase_index: 1,
        at_beat: 6,
    })?;

    assert!(matches!(
        edited.phrases()[1].loop_strategy(),
        PhraseLoopStrategy::FixedVariant(variant) if variant.as_str() == "variant-2"
    ));
    assert_eq!(
        edited.phrases()[2].loop_strategy(),
        &PhraseLoopStrategy::Auto
    );
    assert_eq!(edited.phrases()[1].role_id(), edited.phrases()[2].role_id());
    Ok(())
}

#[test]
fn invalid_boundaries_and_implicit_delete_are_typed_rejections()
-> Result<(), Box<dyn std::error::Error>> {
    let original = timeline()?;
    assert_eq!(
        original.edit(TimelineEditCommand::Split {
            phrase_index: 1,
            at_beat: 4,
        }),
        Err(TimelineEditError::InvalidSplitBoundary)
    );
    assert_eq!(
        original.edit(TimelineEditCommand::MoveBoundary {
            boundary_after_phrase_index: 1,
            to_beat: 4,
        }),
        Err(TimelineEditError::InvalidBoundaryMove)
    );
    assert_eq!(
        original.edit(TimelineEditCommand::Create {
            start_beat: 9,
            end_beat: 9,
            role_id: role("drop")?,
        }),
        Err(TimelineEditError::InvalidBeatSelection)
    );
    assert_eq!(
        original.edit(TimelineEditCommand::Delete {
            phrase_index: 0,
            absorb_into: PhraseAbsorption::Previous,
        }),
        Err(TimelineEditError::MissingPreviousPhrase)
    );
    assert_eq!(
        original.edit(TimelineEditCommand::Create {
            start_beat: 0,
            end_beat: 4,
            role_id: role("intro")?,
        }),
        Err(TimelineEditError::NoChange)
    );
    Ok(())
}

#[test]
fn undo_redo_and_revision_restore_copy_immutable_content() -> Result<(), Box<dyn std::error::Error>>
{
    let initial = timeline()?;
    let changed = initial.edit(TimelineEditCommand::ChangeRole {
        phrase_index: 1,
        role_id: role("synth")?,
    })?;
    let undone = LumiPhraseTimeline::restore(&changed, &initial, TimelineRevisionReason::Undo)?;
    let redone = LumiPhraseTimeline::restore(&undone, &changed, TimelineRevisionReason::Redo)?;

    assert_eq!(undone.phrases(), initial.phrases());
    assert_eq!(undone.restored_from(), Some(initial.revision()));
    assert_eq!(undone.reason(), TimelineRevisionReason::Undo);
    assert_eq!(redone.phrases(), changed.phrases());
    assert_eq!(redone.restored_from(), Some(changed.revision()));
    assert_eq!(redone.reason(), TimelineRevisionReason::Redo);
    assert_eq!(redone.revision(), TimelineRevision::try_new(4)?);
    Ok(())
}

#[test]
fn randomized_valid_edits_never_create_a_gap_overlap_or_zero_length_phrase()
-> Result<(), Box<dyn std::error::Error>> {
    let mut seed = 0x5eed_u64;
    for _ in 0..500 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let mut value = timeline()?;
        for _ in 0..24 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let phrases = value.phrases();
            let index = usize::try_from(seed % u64::try_from(phrases.len())?)?;
            let phrase = &phrases[index];
            let command = match seed % 5 {
                0 if phrase.end_beat() - phrase.start_beat() > 1 => TimelineEditCommand::Split {
                    phrase_index: phrase.index(),
                    at_beat: phrase.start_beat() + 1,
                },
                1 if index + 1 < phrases.len() => TimelineEditCommand::MoveBoundary {
                    boundary_after_phrase_index: phrase.index(),
                    to_beat: phrases[index + 1].end_beat() - 1,
                },
                2 if index > 0 => TimelineEditCommand::MergePrevious {
                    phrase_index: phrase.index(),
                },
                3 if phrases.len() > 1 && index + 1 < phrases.len() => {
                    TimelineEditCommand::Delete {
                        phrase_index: phrase.index(),
                        absorb_into: PhraseAbsorption::Next,
                    }
                }
                _ => TimelineEditCommand::ChangeRole {
                    phrase_index: phrase.index(),
                    role_id: role(if phrase.role_id().as_str() == "synth" {
                        "drop"
                    } else {
                        "synth"
                    })?,
                },
            };
            if let Ok(edited) = value.edit(command) {
                value = edited;
                assert_canonical(&value);
            }
        }
    }
    Ok(())
}

#[test]
fn theme_specific_exact_requires_sorted_unique_theme_overrides()
-> Result<(), Box<dyn std::error::Error>> {
    let invalid = PhraseInstance::new(0, 0, 16, role("drop")?).with_loop_strategy(
        PhraseLoopStrategy::ThemeSpecificExact(vec![
            ThemeSpecificVariant::new(ThemeId::new(2), VariantId::try_new("v2")?),
            ThemeSpecificVariant::new(ThemeId::new(1), VariantId::try_new("v1")?),
        ]),
    );
    assert!(
        LumiPhraseTimeline::try_new(
            TrackId::new(7),
            TimelineRevision::initial(),
            SourceRevision::try_new("analysis-v1")?,
            16,
            TimelineRevisionOrigin::SourceImport,
            vec![invalid],
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn canonical_edit_transcript_is_stable() -> Result<(), Box<dyn std::error::Error>> {
    let mut value = timeline()?;
    let mut transcript = vec![timeline_signature(&value)];
    let commands = [
        TimelineEditCommand::Split {
            phrase_index: 1,
            at_beat: 6,
        },
        TimelineEditCommand::ChangeRole {
            phrase_index: 2,
            role_id: role("synth")?,
        },
        TimelineEditCommand::MoveBoundary {
            boundary_after_phrase_index: 1,
            to_beat: 5,
        },
        TimelineEditCommand::Delete {
            phrase_index: 1,
            absorb_into: PhraseAbsorption::Next,
        },
        TimelineEditCommand::MergePrevious { phrase_index: 2 },
    ];
    for command in commands {
        value = value.edit(command)?;
        transcript.push(timeline_signature(&value));
    }

    assert_eq!(
        transcript.join("\n"),
        "R1 InitialSourceMapping 0-4:intro:auto|4-8:breakdown-1:fixed|8-12:buildup-1:auto|12-16:drop:auto\n\
         R2 SplitPhrase 0-4:intro:auto|4-6:breakdown-1:fixed|6-8:breakdown-1:auto|8-12:buildup-1:auto|12-16:drop:auto\n\
         R3 ChangeRole 0-4:intro:auto|4-6:breakdown-1:fixed|6-8:synth:auto|8-12:buildup-1:auto|12-16:drop:auto\n\
         R4 MoveBoundary 0-4:intro:auto|4-5:breakdown-1:fixed|5-8:synth:auto|8-12:buildup-1:auto|12-16:drop:auto\n\
         R5 AbsorbNext 0-4:intro:auto|4-8:synth:auto|8-12:buildup-1:auto|12-16:drop:auto\n\
         R6 MergePrevious 0-4:intro:auto|4-12:buildup-1:auto|12-16:drop:auto"
    );
    Ok(())
}

fn timeline_signature(value: &LumiPhraseTimeline) -> String {
    let phrases = value
        .phrases()
        .iter()
        .map(|phrase| {
            let strategy = match phrase.loop_strategy() {
                PhraseLoopStrategy::Auto => "auto",
                PhraseLoopStrategy::FixedVariant(_) => "fixed",
                PhraseLoopStrategy::ThemeSpecificExact(_) => "theme-exact",
            };
            format!(
                "{}-{}:{}:{strategy}",
                phrase.start_beat(),
                phrase.end_beat(),
                phrase.role_id().as_str()
            )
        })
        .collect::<Vec<_>>()
        .join("|");
    format!(
        "R{} {:?} {phrases}",
        value.revision().value(),
        value.reason()
    )
}
