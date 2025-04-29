use crate::process_ui::ProcessState;
use ratatui::{prelude::*, widgets::Paragraph};

pub fn telescope(
    tele: Vec<u64>,
    process: &ProcessState,
    include_first: bool,
    prefix: String,
) -> Line {
    let tele_len = tele.len();

    let mut tele: Vec<_> = tele
        .into_iter()
        .enumerate()
        .skip(if include_first { 0 } else { 1 })
        .flat_map(|(i, maybe_addr)| {
            let style = match process.addr_memory_perm(maybe_addr) {
                Some((r, w, x)) => crate::theme::memory_permissions(r, w, x),
                None => Style::default(),
            };

            if i == (tele_len - 1) {
                vec![Span::from(format!("{maybe_addr:#02x}")).style(style)]
            } else if i == 0 {
                vec![
                    Span::from(format!("{maybe_addr:#018x}")).style(style),
                    Span::from(" > ").style(Style::default()),
                ]
            } else {
                vec![
                    Span::from(format!("{maybe_addr:#018x}")).style(style),
                    Span::from(" > ").style(Style::default()),
                ]
            }
        })
        .collect();

    let mut l = vec![Span::from(prefix)];
    l.extend(tele);

    Line::from(l)
}
