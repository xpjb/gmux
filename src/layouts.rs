use crate::state::{Gmux};
use crate::ClientHandle;

#[derive(Debug)]
pub struct Layout {
    pub symbol: &'static str,
    pub arrange: Option<fn(&mut Gmux, usize)>,
}

pub static LAYOUTS: [Layout; 3] = [
    Layout {
        symbol: "[]=",
        arrange: Some(tile),
    },
    Layout {
        symbol: "><>",
        arrange: Some(monocle),
    },
    Layout {
        symbol: "[M]",
        arrange: Some(monocle),
    },
];

fn tile(state: &mut Gmux, mon_idx: usize) {
    let mon = &state.mons[mon_idx];
    let tiled_clients: Vec<ClientHandle> = mon.stack.iter()
        .filter(|h| state.clients.get(h).map_or(false, |c| !c.is_floating && c.is_visible_on(mon)))
        .cloned()
        .collect();

    let n = tiled_clients.len();
    if n == 0 {
        return;
    }

    let nmaster = mon.nmaster;
    let mfact = mon.mfact;
    let ww = mon.ww;
    let wh = mon.wh;
    let wx = mon.wx;
    let wy = mon.wy;

    let mw = if n > nmaster as usize {
        if nmaster > 0 {
            (ww as f32 * mfact) as i32
        } else {
            0
        }
    } else {
        ww
    };

    let mut my = 0;
    let mut ty = 0;

    for (i, &handle) in tiled_clients.iter().enumerate() {
        if let Some(client) = state.clients.get(&handle) {
            let client_bw = client.bw;

            if i < nmaster as usize {
                let h = (wh - my) / (std::cmp::min(n, nmaster as usize) - i) as i32;
                unsafe { state.resize(
                    handle,
                    wx,
                    wy + my,
                    mw - (2 * client_bw),
                    h - (2 * client_bw),
                ) };
                if my + h < wh {
                    my += h;
                }
            } else {
                let h = (wh - ty) / (n - i) as i32;
                state.resize(
                    handle,
                    wx + mw,
                    wy + ty,
                    ww - mw - (2 * client_bw),
                    h - (2 * client_bw),
                );
                if ty + h < wh {
                    ty += h;
                }
            }
        }
    }
}

fn monocle(state: &mut Gmux, mon_idx: usize) {
    let mon = &state.mons[mon_idx];
    let tiled_clients: Vec<ClientHandle> = mon.stack.iter()
        .filter(|h| state.clients.get(h).map_or(false, |c| !c.is_floating && c.is_visible_on(mon)))
        .cloned()
        .collect();

    let wx = mon.wx;
    let wy = mon.wy;
    let ww = mon.ww;
    let wh = mon.wh;

    for &handle in &tiled_clients {
        if let Some(client) = state.clients.get(&handle) {
            let client_bw = client.bw;
            state.resize(
                handle,
                wx,
                wy,
                ww - 2 * client_bw,
                wh - 2 * client_bw,
            );
        }
    }
}
