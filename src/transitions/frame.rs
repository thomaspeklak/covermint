use gtk::prelude::*;

use crate::{
    model::{Placement, Transition},
    motion::{ease_in_out_cubic, ease_out_back_subtle},
    ui::ArtworkLayer,
};

#[derive(Clone, Copy)]
pub(super) struct TransitionFrame {
    pub(super) width_progress: f64,
    pub(super) height_progress: f64,
    pub(super) offset_x: f64,
    pub(super) offset_y: f64,
    pub(super) opacity: f64,
}

pub(super) fn bring_artwork_to_front(stack: &gtk::Fixed, artwork: &ArtworkLayer) {
    stack.remove(&artwork.stage);
    stack.put(&artwork.stage, 0.0, 0.0);
}

pub(super) fn reset_artwork_frame(artwork: &ArtworkLayer, width: i32, height: i32) {
    artwork.picture.set_size_request(width, height);
    artwork.stage.move_(&artwork.picture, 0.0, 0.0);
}

pub(super) fn render_picture_frame(
    artwork: &ArtworkLayer,
    width: i32,
    height: i32,
    transition: Transition,
    frame: TransitionFrame,
) {
    let (frame_width, frame_height, x, y) = match transition {
        Transition::None | Transition::Fade => (width, height, 0.0, 0.0),
        Transition::Flip => {
            let frame_width = scaled_frame_size(width, frame.width_progress, 1.08);
            (frame_width, height, ((width - frame_width) / 2) as f64, 0.0)
        }
        Transition::Hinge => {
            let frame_width = scaled_frame_size(width, frame.width_progress, 1.04);
            let frame_height = scaled_frame_size(height, frame.height_progress, 1.04);
            (
                frame_width,
                frame_height,
                ((width - frame_width) / 2) as f64,
                0.0,
            )
        }
        Transition::Slide | Transition::Cover => (
            width,
            height,
            (width as f64 * frame.offset_x).round(),
            (height as f64 * frame.offset_y).round(),
        ),
    };

    artwork.picture.set_size_request(frame_width, frame_height);
    artwork.stage.move_(&artwork.picture, x, y);
    artwork.picture.set_opacity(frame.opacity.clamp(0.0, 1.0));
}

pub(super) fn transition_frames(
    transition: Transition,
    placement: Placement,
    progress: f64,
) -> (TransitionFrame, TransitionFrame) {
    let t = progress.clamp(0.0, 1.0);

    match transition {
        Transition::None => (
            TransitionFrame {
                width_progress: 1.0,
                height_progress: 1.0,
                offset_x: 0.0,
                offset_y: 0.0,
                opacity: 0.0,
            },
            TransitionFrame {
                width_progress: 1.0,
                height_progress: 1.0,
                offset_x: 0.0,
                offset_y: 0.0,
                opacity: 1.0,
            },
        ),
        Transition::Fade => {
            let eased = ease_in_out_cubic(t);
            (
                TransitionFrame {
                    width_progress: 1.0,
                    height_progress: 1.0,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    opacity: 1.0 - eased,
                },
                TransitionFrame {
                    width_progress: 1.0,
                    height_progress: 1.0,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    opacity: eased,
                },
            )
        }
        Transition::Flip => {
            if t < 0.5 {
                let phase = ease_in_out_cubic(t / 0.5);
                (
                    TransitionFrame {
                        width_progress: 1.0 - phase,
                        height_progress: 1.0,
                        offset_x: 0.0,
                        offset_y: 0.0,
                        opacity: 1.0 - (phase * 0.85),
                    },
                    TransitionFrame {
                        width_progress: 0.0,
                        height_progress: 1.0,
                        offset_x: 0.0,
                        offset_y: 0.0,
                        opacity: 0.0,
                    },
                )
            } else {
                let phase = (t - 0.5) / 0.5;
                (
                    TransitionFrame {
                        width_progress: 0.0,
                        height_progress: 1.0,
                        offset_x: 0.0,
                        offset_y: 0.0,
                        opacity: 0.0,
                    },
                    TransitionFrame {
                        width_progress: ease_out_back_subtle(phase),
                        height_progress: 1.0,
                        offset_x: 0.0,
                        offset_y: 0.0,
                        opacity: ease_in_out_cubic(phase),
                    },
                )
            }
        }
        Transition::Hinge => {
            if t < 0.5 {
                let phase = ease_in_out_cubic(t / 0.5);
                (
                    TransitionFrame {
                        width_progress: 1.0 - (phase * 0.72),
                        height_progress: 1.0 - (phase * 0.16),
                        offset_x: 0.0,
                        offset_y: 0.0,
                        opacity: 1.0 - (phase * 0.9),
                    },
                    TransitionFrame {
                        width_progress: 0.28,
                        height_progress: 0.84,
                        offset_x: 0.0,
                        offset_y: 0.0,
                        opacity: 0.0,
                    },
                )
            } else {
                let phase = (t - 0.5) / 0.5;
                let spring = ease_out_back_subtle(phase);
                (
                    TransitionFrame {
                        width_progress: 0.28,
                        height_progress: 0.84,
                        offset_x: 0.0,
                        offset_y: 0.0,
                        opacity: 0.0,
                    },
                    TransitionFrame {
                        width_progress: 0.28 + (spring * 0.72),
                        height_progress: 0.84 + (spring * 0.16),
                        offset_x: 0.0,
                        offset_y: 0.0,
                        opacity: ease_in_out_cubic(phase),
                    },
                )
            }
        }
        Transition::Slide => {
            let eased = ease_in_out_cubic(t);
            let (direction_x, direction_y) = placement
                .edge_motion_direction()
                .expect("validated slide transition placement");
            (
                TransitionFrame {
                    width_progress: 1.0,
                    height_progress: 1.0,
                    offset_x: direction_x * eased,
                    offset_y: direction_y * eased,
                    opacity: 1.0 - eased,
                },
                TransitionFrame {
                    width_progress: 1.0,
                    height_progress: 1.0,
                    offset_x: -direction_x * (1.0 - eased),
                    offset_y: -direction_y * (1.0 - eased),
                    opacity: 1.0,
                },
            )
        }
        Transition::Cover => {
            let eased = ease_in_out_cubic(t);
            let (direction_x, direction_y) = placement
                .edge_motion_direction()
                .expect("validated cover transition placement");
            (
                TransitionFrame {
                    width_progress: 1.0,
                    height_progress: 1.0,
                    offset_x: direction_x * eased,
                    offset_y: direction_y * eased,
                    opacity: 1.0 - eased,
                },
                TransitionFrame {
                    width_progress: 1.0,
                    height_progress: 1.0,
                    offset_x: direction_x * (1.0 - eased),
                    offset_y: direction_y * (1.0 - eased),
                    opacity: 1.0,
                },
            )
        }
    }
}

fn scaled_frame_size(size: i32, progress: f64, max_progress: f64) -> i32 {
    ((size as f64 * progress.clamp(0.0, max_progress)).round() as i32).max(1)
}
