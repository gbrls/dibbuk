use crate::{process_ui::ProcessState, tui::ViewOptions};
use color_eyre::owo_colors::OwoColorize;
use ratatui::{prelude::*, widgets::Paragraph};

pub fn art_u8(value: u8) -> String {
    let a = value & 0xf;
    let b = (value >> 4) & 0xf;
    //let s = "░▒▓";
    let s = " ▖▗▄▘▌▚▙▝▞▐▟▀▛▜█";
    let braile_blank = 0x2800;
    let chrs = [
        '░', '▖', '▗', '▄', '▘', '▌', '▚', '▙', '▝', '▞', '▐', '▟', '▀', '▛', '▜', '█',
    ];

    let a = chrs[a as usize];
    let b = chrs[b as usize];

    let chr = if value > 0 {
        char::from_u32(braile_blank + value as u32).unwrap()
    } else {
        '░'
    };

    format!("{b}{a}")
    //format!("{chr}")
}

pub fn art_u32(value: u32) -> String {
    let a = art_u8((value & 0xff) as u8);
    let b = art_u8(((value >> 8) & 0xff) as u8);
    let c = art_u8(((value >> 16) & 0xff) as u8);
    let d = art_u8(((value >> 24) & 0xff) as u8);
    format!("{d}{c}{b}{a}")
}

pub fn art_u64(value: u64) -> String {
    let a = art_u32((value & 0xffff_ffff) as u32);
    let b = art_u32(((value >> 32) & 0xffff_ffff) as u32);

    format!("{b}{a}")
}

pub fn display_u64(
    value: u64,
    process: &ProcessState,
    view_options: &ViewOptions,
) -> Span<'static> {
    let art = art_u64(value);
    if view_options.goblin_mode {
        Span::from(format!("{art}")).style(memory_style(value, process))
    } else {
        Span::from(format!("{value:#018x}")).style(memory_style(value, process))
    }
}

pub fn memory_style(addr: u64, process: &ProcessState) -> Style {
    match process.addr_memory_perm(addr) {
        Some((r, w, x)) => crate::theme::memory_permissions(r, w, x),
        None => Style::default(),
    }
}

pub fn telescope(
    tele: Vec<u64>,
    process: &ProcessState,
    view_options: &ViewOptions,
    include_first: bool,
    prefix: String,
) -> Line<'static> {
    let tele_len = tele.len();

    let mut tele: Vec<_> = tele
        .into_iter()
        .enumerate()
        .skip(if include_first { 0 } else { 1 })
        .flat_map(|(i, maybe_addr)| {
            if i == (tele_len - 1) {
                let display_guess =
                    guess_display(maybe_addr, process, view_options).unwrap_or("".into());
                vec![Span::from(format!("{maybe_addr:#02x} {display_guess}"))
                    .style(memory_style(maybe_addr, process))]
            } else if i == 0 {
                vec![
                    display_u64(maybe_addr, process, view_options),
                    Span::from(" ➝ ").style(Style::default()),
                ]
            } else {
                vec![
                    display_u64(maybe_addr, process, view_options),
                    Span::from(" ➝ ").style(Style::default()),
                ]
            }
        })
        .collect();

    let mut l = vec![Span::from(prefix)];
    l.extend(tele);

    Line::from(l)
}

pub fn guess_display(
    val: u64,
    process: &ProcessState,
    view_options: &ViewOptions,
) -> Option<String> {
    let bytes = val.to_le_bytes();

    let all_ascii = bytes.iter().all(|&b| b.is_ascii());
    if all_ascii {
        let valid_bytes: Vec<u8> = bytes.iter().filter(|&&b| b != 0).cloned().collect();

        let mut ascii_string = String::from_utf8_lossy(&valid_bytes).into_owned();

        if !ascii_string.is_empty() {
            ascii_string.push_str("...");
            Some(ascii_string)
        } else {
            None
        }
    } else {
        None
    }
}
