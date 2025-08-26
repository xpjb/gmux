use crate::state::{Gmux};
use crate::resize;
use crate::is_visible;

#[derive(Debug)]
pub struct Layout {
    pub symbol: &'static str,
    pub arrange: Option<unsafe extern "C" fn(&mut Gmux, usize)>,
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

unsafe extern "C" fn tile(state: &mut Gmux, mon_idx: usize) {
    let mon = &state.mons[mon_idx];
    let tiled_client_indices: Vec<usize> = mon
        .clients
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.is_floating && is_visible(c, mon))
        .map(|(i, _)| i)
        .collect();
    let n = tiled_client_indices.len();
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

    for (i, &client_idx) in tiled_client_indices.iter().enumerate() {
        let client_bw = state.mons[mon_idx].clients[client_idx].bw;

        if i < nmaster as usize {
            let h = (wh - my) / (std::cmp::min(n, nmaster as usize) - i) as i32;
            unsafe { resize(
                state,
                mon_idx,
                client_idx,
                wx,
                wy + my,
                mw - (2 * client_bw),
                h - (2 * client_bw),
                false,
            ) };
            if my + h < wh {
                my += h;
            }
        } else {
            let h = (wh - ty) / (n - i) as i32;
            unsafe { resize(
                state,
                mon_idx,
                client_idx,
                wx + mw,
                wy + ty,
                ww - mw - (2 * client_bw),
                h - (2 * client_bw),
                false,
            ) };
            if ty + h < wh {
                ty += h;
            }
        }
    }
}

unsafe extern "C" fn monocle(state: &mut Gmux, mon_idx: usize) {
    let mon = &state.mons[mon_idx];
    let tiled_client_indices: Vec<usize> = mon
        .clients
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.is_floating && is_visible(c, mon))
        .map(|(i, _)| i)
        .collect();

    let wx = mon.wx;
    let wy = mon.wy;
    let ww = mon.ww;
    let wh = mon.wh;

    for &client_idx in &tiled_client_indices {
        let client_bw = state.mons[mon_idx].clients[client_idx].bw;
        unsafe { resize(
            state,
            mon_idx,
            client_idx,
            wx,
            wy,
            ww - 2 * client_bw,
            wh - 2 * client_bw,
            false,
        ) };
    }
}
