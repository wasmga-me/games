//! The game's own picture over the 3D scene: the level and how far to its
//! boss (or the distance, endless), the score, and a word in the middle as
//! things happen. Drawn on a clear screen in overlay mode, so the page lays
//! it over the scene; only redrawn when what it shows changes, and then only
//! the band at the top it draws in, which is all the page uploads and shows.
use crate::*;

/// how long a banner shows, ms
const BANNER_MS: i32 = 1900;
/// how far down the top line reaches
const TOP_BAND: f32 = 50.0;

pub fn level_text(n: i32) -> String {
    match wg::lang() {
        wg::ZH => format!("第 {n} 关"),
        wg::ZT => format!("第 {n} 關"),
        wg::JA => format!("レベル {n}"),
        wg::KO => format!("레벨 {n}"),
        wg::ES => format!("Nivel {n}"),
        wg::PT => format!("Nível {n}"),
        wg::FR => format!("Niveau {n}"),
        wg::RU => format!("Уровень {n}"),
        _ => format!("Level {n}"),
    }
}

fn banner_text(p: &Combat) -> String {
    let s = &p.s;
    match s.banner {
        B_LEVEL => level_text(s.banner_v),
        B_BOSS => wg::tr(&[
            "Boss", "首领", "ボス", "보스", "Jefe", "Chefe", "Boss", "Boss", "Босс", "首領",
        ])
        .into(),
        B_CLEAR if s.endless() => wg::tr(&[
            "Boss defeated",
            "击败首领",
            "ボス撃破",
            "보스 격파",
            "Jefe derrotado",
            "Chefe derrotado",
            "Boss vaincu",
            "Boss besiegt",
            "Босс побеждён",
            "擊敗首領",
        ])
        .into(),
        B_CLEAR => wg::tr(&[
            "Level cleared",
            "过关",
            "レベルクリア",
            "레벨 클리어",
            "Nivel superado",
            "Nível concluído",
            "Niveau terminé",
            "Level geschafft",
            "Уровень пройден",
            "過關",
        ])
        .into(),
        B_GUN => wg::tr(&WEAPONS[s.banner_v.clamp(0, 4) as usize].names).into(),
        _ => String::new(),
    }
}

/// How far the squad is to the boss, 0 to 1.
fn progress(s: &Save) -> f32 {
    if s.phase != P_RUN {
        return 1.0;
    }
    // before the boss is laid, the pier laid so far is most of the way
    let end = if s.stop_d > 0.0 {
        s.stop_d
    } else {
        s.next_d + 40.0
    };
    (s.d / end.max(1.0)).clamp(0.0, 1.0)
}

/// The boss's health left, 0 to 1, while it is fought.
fn boss_left(s: &Save) -> Option<f32> {
    (s.phase == P_BOSS && s.boss.state == 2).then(|| s.boss.hp as f32 / s.boss.hp_max.max(1) as f32)
}

fn banner_alpha(s: &Save) -> f32 {
    if s.banner == B_NONE || s.banner_ms >= BANNER_MS {
        return 0.0;
    }
    let t = s.banner_ms as f32 / BANNER_MS as f32;
    (t / 0.12).min(1.0).min((1.0 - t) / 0.25)
}

pub fn picture(p: &Combat) -> u32 {
    let s = &p.s;
    let shown = [
        s.level,
        s.score,
        if s.endless() {
            s.d as i32
        } else {
            (progress(s) * 160.0) as i32
        },
        s.banner,
        s.banner_v,
        (banner_alpha(s) * 40.0) as i32,
        s.phase,
        boss_left(s).map_or(-1, |f| (f * 160.0) as i32),
    ];
    wg::hash(0, wg::bytes_of(&shown))
}

/// Text with a soft shadow under it, so it reads over the sea and the sky.
fn text(s: &mut Screen, t: &str, x: f32, y: f32, size: f32, align: i32, alpha: f32) {
    let ink = wg::hex(0xffffff);
    let dark = wg::hex(0x10141c);
    for (dx, dy, c, a) in [
        (0.0, size * 0.08, dark, 0.45 * alpha),
        (0.0, 0.0, ink, alpha),
    ] {
        match align {
            0 => {
                s.text(t, x + dx, y + dy, size, c, a);
            }
            1 => s.text_c(t, x + dx, y + dy, size, c, a),
            _ => s.text_r(t, x + dx, y + dy, size, c, a),
        }
    }
}

pub fn draw(p: &mut Combat, s: &mut Screen) {
    let (x0, y0, x1, y1) = s.visible();
    let w = x1 - x0;
    let a = banner_alpha(&p.s);
    // the band drawn in: the top line, and the banner below it while one shows
    let band = if a > 0.0 {
        (y1 - y0) * 0.3 + 30.0
    } else {
        TOP_BAND
    };
    p.scene.hud_rows = (s.px(0.0, y0 + band).1.ceil() as i32).clamp(1, s.size_px().1) as u32;
    s.overlay(true);
    s.set_clip(x0, y0, w, band);
    s.clear(0);
    let st = &p.s;
    let top = y0 + 26.0;
    let left = if st.endless() {
        format!("{} m", st.d.max(0.0) as i32)
    } else {
        level_text(st.level + 1)
    };
    text(s, &left, x0 + 16.0, top, 17.0, 0, 1.0);
    text(s, &st.score.to_string(), x1 - 16.0, top, 21.0, 2, 1.0);
    // how far to the boss, then how much the boss has left
    let bw = (w * 0.34).min(180.0);
    let bx = x0 + (w - bw) / 2.0;
    let by = top - 4.0;
    let boss = boss_left(st);
    if boss.is_some() || !st.endless() {
        s.rect(bx, by, bw, 8.0, 4.0, wg::hex(0x10141c), 0.4);
    }
    if let Some(f) = boss {
        if f > 0.0 {
            s.rect(bx, by, (bw * f).max(8.0), 8.0, 4.0, wg::hex(0xe8402a), 0.95);
        }
    } else if !st.endless() {
        let f = progress(st);
        if f > 0.0 {
            s.rect(bx, by, (bw * f).max(8.0), 8.0, 4.0, wg::hex(0x8fdc5a), 0.95);
        }
        s.disc(bx + bw + 9.0, by + 4.0, 6.0, wg::hex(0xe8402a), 0.95);
    }
    if a > 0.0 {
        let t = banner_text(p);
        let size = s.fit_size(&t, 36.0, w - 40.0);
        text(s, &t, x0 + w / 2.0, y0 + (y1 - y0) * 0.3, size, 1, a);
    }
    s.no_clip();
}
