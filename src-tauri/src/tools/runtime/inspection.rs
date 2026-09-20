use std::path::Path;

use crate::tools::server_tools::inspect_gepard;

use super::compatibility::SubjectObservation;

pub(crate) fn inspect_subject(game_dir: Option<&Path>) -> SubjectObservation {
    let Some(game_dir) = game_dir else {
        return SubjectObservation::Absent;
    };
    let Some(inspection) = inspect_gepard(game_dir) else {
        return SubjectObservation::Absent;
    };
    match inspection.sha256 {
        Some(sha256) => SubjectObservation::Hash {
            sha256_lowercase: sha256.to_ascii_lowercase(),
        },
        None => SubjectObservation::Unreadable,
    }
}
