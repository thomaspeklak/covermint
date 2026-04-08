use crate::{
    model::{MediaState, TrackMetadata},
    mpris,
};

const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";

pub(crate) fn list_players() {
    let players = mpris::player_names();
    if players.is_empty() {
        eprintln!("covermint: no MPRIS players were found on the session bus");
        return;
    }

    println!("{}", players.join("\n"));
}

pub(crate) fn query_player(player: &str, include_metadata: bool) -> Option<MediaState> {
    let players = mpris::snapshot();

    let selected = if player.eq_ignore_ascii_case("auto") {
        players
            .values()
            .max_by_key(|state| (state.status.auto_select_rank(), state.art_url.is_some()))
            .cloned()
    } else {
        players.iter().find_map(|(name, state)| {
            if selector_matches_player(name, player) {
                Some(state.clone())
            } else {
                None
            }
        })
    };

    selected.map(|mut state| {
        if !include_metadata {
            state.metadata = TrackMetadata::default();
        }
        state
    })
}

fn selector_matches_player(player_name: &str, selector: &str) -> bool {
    player_name.eq_ignore_ascii_case(selector)
        || format!("{MPRIS_PREFIX}{player_name}").eq_ignore_ascii_case(selector)
}
