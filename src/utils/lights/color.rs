

#[allow(non_snake_case, dead_code)]
pub fn rgb_to_xyb(rgb: &[u16; 3]) -> [f32; 3] {
    let mut rgb: [f32; 3] = rgb
        .iter()
        .map(|v| *v as f32 / u16::MAX as f32)
        .collect::<Vec<f32>>()
        .try_into()
        .unwrap();
    rgb.iter_mut().for_each(|v| {
        *v = if *v > 0.04045 {
            ((*v + 0.055) / 1.055).powf(2.4)
        } else {
            *v / 12.92
        }
    });

    let X = rgb[0] * 0.4124 + rgb[1] * 0.3576 + rgb[2] * 0.1805;
    let Y = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
    let Z = rgb[0] * 0.0193 + rgb[1] * 0.1192 + rgb[2] * 0.9505;

    let x = X / (X + Y + Z);
    let y = Y / (X + Y + Z);

    [x, y, Y]
}

#[allow(non_snake_case, dead_code)]
pub fn xyb_to_rgb(xyb: &[f32; 3]) -> [u16; 3] {
    let x = xyb[0];
    let y = xyb[1];
    let z = 1.0 - x - y;
    let Y = xyb[2];
    let X = (Y / y) * x;
    let Z = (Y / y) * z;
    let mut r = X * 3.2406 - Y * 1.537 - Z * 0.4986;
    let mut g = -X * 0.9689 + Y * 1.8758 + Z * 0.0415;
    let mut b = X * 0.0557 - Y * 0.2040 + Z * 1.0570;
    r = if r <= 0.0031308 {
        12.92 * r
    } else {
        (1.0 + 0.055) * r.powf(1.0 / 2.4) - 0.055
    };
    g = if g <= 0.0031308 {
        12.92 * g
    } else {
        (1.0 + 0.055) * g.powf(1.0 / 2.4) - 0.055
    };
    b = if b <= 0.0031308 {
        12.92 * b
    } else {
        (1.0 + 0.055) * b.powf(1.0 / 2.4) - 0.055
    };
    [
        (r * u16::MAX as f32) as u16,
        (g * u16::MAX as f32) as u16,
        (b * u16::MAX as f32) as u16,
    ]
}

pub fn rgb_to_hsv(rgb: &[u16; 3]) -> [f32; 3] {
    let out: [f32; 3] = [
        rgb[0] as f32 / u16::MAX as f32,
        rgb[1] as f32 / u16::MAX as f32,
        rgb[2] as f32 / u16::MAX as f32,
    ];
    let c_max = out.iter().reduce(|a, b| if a > b { a } else { b }).unwrap();
    let c_min = out.iter().reduce(|a, b| if a < b { a } else { b }).unwrap();
    let delta = c_max - c_min;

    let h: f32;
    if delta == 0.0 {
        h = 0.0
    } else {
        match c_max {
            i if out[0] == *i => {
                let check = 60.0 * (((out[1] - out[2]) / delta) % 6.0);
                h = if check >= 0.0 { check } else { 360.0 + check };
            }
            i if out[1] == *i => h = 60.0 * (((out[2] - out[0]) / delta) + 2.0),

            i if out[2] == *i => h = 60.0 * (((out[0] - out[1]) / delta) + 4.0),
            _ => h = 0.0,
        }
    }

    let s = if *c_max == 0.0 { 0.0 } else { delta / c_max };

    [h, s, *c_max]
}

pub fn hsv_to_rgb(hsv: &[f32; 3]) -> [u16; 3] {
    let c = hsv[2] * hsv[1];
    let x = c * (1.0 - ((hsv[0] / 60.0) % 2.0 - 1.0).abs());
    let m = hsv[2] - c;

    let (r, g, b) = match hsv[0] {
        h if h < 60.0 => (c, x, 0.0),
        h if h < 120.0 => (x, c, 0.0),
        h if h < 180.0 => (0.0, c, x),
        h if h < 240.0 => (0.0, x, c),
        h if h < 300.0 => (x, 0.0, c),
        h if h < 360.0 => (c, 0.0, x),
        _ => (0.0, 0.0, 0.0),
    };

    let r = (r + m) * u16::MAX as f32;
    let g = (g + m) * u16::MAX as f32;
    let b = (b + m) * u16::MAX as f32;

    [r as u16, g as u16, b as u16]
}

pub fn interpolate_hsv(a: &[f32; 3], b: &[f32; 3], t: f32) -> [f32; 3] {
    let h = a[0] + t * (b[0] - a[0]);
    let s = a[1] + t * (b[1] - a[1]);
    let v = a[2] + t * (b[2] - a[2]);

    [h, s, v]
}