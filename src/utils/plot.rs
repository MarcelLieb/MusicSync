use std::collections::HashMap;

use plotters::{
    prelude::{
        BitMapBackend, ChartBuilder, Circle, IntoDrawingArea, LabelAreaPosition, Rectangle,
        SeriesLabelPosition,
    },
    style::{AsRelative, Color, Palette, Palette99, BLACK, WHITE},
};

use super::lights::Event;

const TIME_WINDOW: u128 = 10000;

pub fn plot(
    data: &HashMap<String, Vec<(u128, Event)>>,
    file: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = BitMapBackend::new(&file, (1920, 1080)).into_drawing_area();

    root.fill(&WHITE)?;

    let max = data.iter().fold(0_u128, |acc, (_, vec)| {
        vec.iter()
            .filter(|(t, _)| *t < TIME_WINDOW)
            .last()
            .unwrap_or(&(0, Event::Full(0.0)))
            .0
            .max(acc)
    });

    let mut chart = ChartBuilder::on(&root)
        .set_label_area_size(LabelAreaPosition::Bottom, (4).percent())
        .margin(20)
        .build_cartesian_2d(0..max, 0_u32..6_u32)?;
    chart
        .configure_mesh()
        .disable_y_mesh()
        .x_desc("time in ms")
        .draw()?;

    let mut keys = data.keys().map(|s| s.to_string()).collect::<Vec<String>>();
    keys.sort();

    let data_max: HashMap<String, f32> = data
        .iter()
        .map(|(key, vec)| {
            (
                key.to_string(),
                vec.iter()
                    .map(|(_, event)| event)
                    .map(|event| match event {
                        Event::Full(y) => *y,
                        Event::Atmosphere(y, _) => *y,
                        Event::Note(y, _) => *y,
                        Event::Drum(y) => *y,
                        Event::Hihat(y) => *y,
                    })
                    .fold(0.0_f32, |acc, x| acc.max(x)),
            )
        })
        .collect();

    for (index, key) in keys.iter().enumerate() {
        let color = Palette99::pick(index);
        chart
            .draw_series({
                data[key]
                    .iter()
                    .map(|(time, event)| match event {
                        Event::Full(y) => (*time, *y),
                        Event::Atmosphere(y, _) => (*time, *y),
                        Event::Note(y, _) => (*time, *y),
                        Event::Drum(y) => (*time, *y),
                        Event::Hihat(y) => (*time, *y),
                    })
                    .map(|(time, y)| (time, y / data_max[key]))
                    .filter(|(t, _)| *t < TIME_WINDOW)
                    .map(|(t, v)| {
                        [
                            Circle::new(
                                (t, (-(index as i32) + 5) as u32),
                                25.0 * v,
                                &color.mix(0.8),
                            ),
                            Circle::new(
                                (t, (-(index as i32) + 5) as u32),
                                2.0,
                                *&color.mix(0.1).filled(),
                            ),
                        ]
                    })
                    .flatten()
            })?
            .label(key)
            .legend(move |(x, y)| Rectangle::new([(x, y - 5), (x + 10, y + 5)], color.filled()));
    }
    chart
        .configure_series_labels()
        .position(SeriesLabelPosition::UpperRight)
        .background_style(&WHITE)
        .border_style(&BLACK)
        .draw()?;

    root.present()?;

    Ok(())
}
