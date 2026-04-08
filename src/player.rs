use crate::{
    model::{MediaState, TrackMetadata},
    mpris,
    timestamp::format_timestamp_microseconds,
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
            .into_iter()
            .max_by_key(|(_, state)| (state.status.auto_select_rank(), state.art_url.is_some()))
    } else {
        players
            .into_iter()
            .find(|(name, _)| selector_matches_player(name, player))
    };

    selected.map(|(player_name, mut state)| {
        if include_metadata {
            if let Some(position_microseconds) = mpris::position_microseconds_now(&player_name) {
                state.metadata.position_microseconds = Some(position_microseconds);
                state.metadata.position = format_timestamp_microseconds(position_microseconds);
            }
        } else {
            state.metadata = TrackMetadata::default();
        }
        state
    })
}

fn selector_matches_player(player_name: &str, selector: &str) -> bool {
    player_name.eq_ignore_ascii_case(selector)
        || format!("{MPRIS_PREFIX}{player_name}").eq_ignore_ascii_case(selector)
}
